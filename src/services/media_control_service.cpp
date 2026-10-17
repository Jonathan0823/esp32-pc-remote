#include <Arduino.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <ArduinoJson.h>
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>
#include <esp_random.h>
#include <esp_heap_caps.h>
#include <stdio.h>
#include <string.h>
#include <memory>
#include <new>
#include <sys/time.h>
#include <time.h>

#include "config.h"
#include "src/services/media_control_config.h"
#include "src/services/media_control_policy.h"
#include "src/services/media_control_service.h"
#include "src/services/indicator_service.h"
#include "src/services/voice_service.h"

namespace {
constexpr time_t MIN_VALID_UNIX_TIME = 1735689600;  // 2025-01-01 UTC
constexpr UBaseType_t MEDIA_QUEUE_LENGTH = 1;
constexpr uint32_t MEDIA_WORKER_STACK_BYTES = 12288;
constexpr uint32_t MEDIA_CONNECT_TIMEOUT_MS = 1000;
constexpr uint32_t MEDIA_READ_TIMEOUT_MS = 1000;
constexpr unsigned long MEDIA_HANDSHAKE_TIMEOUT_SECONDS = 2;

enum class IntentKind : uint8_t { Media, CustomAction, SyncVoiceConfig };

struct MediaIntent {
  IntentKind kind;
  MediaCommand command;
  uint32_t voiceRevision;
  uint32_t recognizedAtMs;
  uint64_t issuedAtUnixMs;
  char id[33];
  char actionId[VOICE_ACTION_ID_MAX_BYTES + 1];
};

QueueHandle_t mediaQueue = nullptr;
bool mediaConfigured = false;

bool unix_time_ms(uint64_t& milliseconds) {
  timeval now = {};
  if (gettimeofday(&now, nullptr) != 0 || now.tv_sec < MIN_VALID_UNIX_TIME) return false;
  milliseconds = static_cast<uint64_t>(now.tv_sec) * 1000ULL + now.tv_usec / 1000ULL;
  return true;
}

void make_request_id(char id[33]) {
  static const char HEX_DIGITS[] = "0123456789abcdef";
  uint8_t random[16];
  esp_fill_random(random, sizeof(random));
  for (size_t i = 0; i < sizeof(random); ++i) {
    id[i * 2] = HEX_DIGITS[random[i] >> 4];
    id[i * 2 + 1] = HEX_DIGITS[random[i] & 0x0f];
  }
  id[32] = '\0';
}

void report_result(MediaResult result, const MediaIntent& intent, int httpStatus = 0) {
  indicator_media_result(result);
  switch (result) {
    case MediaResult::Queued:
      printf("[media] queued cmd=%s id=%s\n", media_command_name(intent.command), intent.id);
      break;
    case MediaResult::Accepted:
      printf("[media] accepted cmd=%s id=%s status=%d; execution not confirmed\n",
             media_command_name(intent.command), intent.id, httpStatus);
      break;
    case MediaResult::Executed:
      printf("[media] executed acknowledgement cmd=%s id=%s\n",
             media_command_name(intent.command), intent.id);
      break;
    case MediaResult::Rejected:
      printf("[media] rejected cmd=%s id=%s status=%d\n",
             media_command_name(intent.command), intent.id, httpStatus);
      break;
    case MediaResult::Unknown:
      printf("[media] outcome unknown cmd=%s id=%s status=%d; not retrying\n",
             media_command_name(intent.command), intent.id, httpStatus);
      break;
  }
}

MediaResult send_intent(const MediaIntent& intent, int& httpStatus) {
  uint64_t nowUnixMs = 0;
  if (!media_request_is_fresh(intent.recognizedAtMs, millis()) ||
      !unix_time_ms(nowUnixMs) ||
      !media_wall_clock_valid(intent.issuedAtUnixMs,
                              intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS,
                              nowUnixMs) ||
      WiFi.status() != WL_CONNECTED) {
    return MediaResult::Rejected;
  }

  StaticJsonDocument<256> request;
  request["id"] = intent.id;
  request["cmd"] = media_command_name(intent.command);
  request["issued_at_ms"] = intent.issuedAtUnixMs;
  request["expires_at_ms"] = intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS;
  char payload[MEDIA_REQUEST_BODY_MAX + 1];
  const size_t payloadLength = serializeJson(request, payload, sizeof(payload));
  if (payloadLength == 0 || payloadLength > MEDIA_REQUEST_BODY_MAX) return MediaResult::Rejected;

  WiFiClientSecure client;
  if (!media_control_prepare_tls(client)) return MediaResult::Rejected;

  HTTPClient http;
  char url[320];
  const int urlLength = snprintf(url, sizeof(url), "https://%s:%u/v1/media",
                                 PC_MEDIA_HOST, static_cast<unsigned>(PC_MEDIA_PORT));
  if (urlLength <= 0 || static_cast<size_t>(urlLength) >= sizeof(url)) return MediaResult::Rejected;
  if (!http.begin(client, url)) return MediaResult::Unknown;

  http.setReuse(false);
  http.setFollowRedirects(HTTPC_DISABLE_FOLLOW_REDIRECTS);
  http.setConnectTimeout(MEDIA_CONNECT_TIMEOUT_MS);
  http.setTimeout(MEDIA_READ_TIMEOUT_MS);
  http.setAcceptEncoding("identity");
  http.addHeader("Content-Type", "application/json");
  const String authorization = String("Bearer ") + PC_MEDIA_API_TOKEN;
  http.addHeader("Authorization", authorization);

  // Recheck expiry before starting the blocking connect/handshake/write path.
  if (!media_request_is_fresh(intent.recognizedAtMs, millis())) {
    http.end();
    return MediaResult::Rejected;
  }
  uint64_t sendTimeMs = 0;
  if (!unix_time_ms(sendTimeMs) ||
      !media_wall_clock_valid(intent.issuedAtUnixMs,
                              intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS,
                              sendTimeMs)) {
    http.end();
    return MediaResult::Rejected;
  }

  multi_heap_info_t internalBefore = {}, psramBefore = {};
  heap_caps_get_info(&internalBefore, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
  heap_caps_get_info(&psramBefore, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
  httpStatus = http.POST(reinterpret_cast<uint8_t*>(payload), payloadLength);
  if (httpStatus < 0) {
    char tlsDetail[160] = {};
    const int tlsError = client.lastError(tlsDetail, sizeof(tlsDetail));
    printf("[media] transport failure cmd=%s id=%s http=%d (%s) tls=%d (%s)\n",
           media_command_name(intent.command), intent.id, httpStatus,
           HTTPClient::errorToString(httpStatus).c_str(), tlsError,
           tlsError == 0 ? "no TLS error recorded" : tlsDetail);
    printf("[media] heap before POST id=%s internal_free=%u internal_largest=%u "
           "psram_free=%u psram_largest=%u\n", intent.id,
           static_cast<unsigned>(internalBefore.total_free_bytes),
           static_cast<unsigned>(internalBefore.largest_free_block),
           static_cast<unsigned>(psramBefore.total_free_bytes),
           static_cast<unsigned>(psramBefore.largest_free_block));
  }
  if (httpStatus >= 400 && httpStatus < 500) {
    http.end();
    return MediaResult::Rejected;
  }
  if (httpStatus != 200 && httpStatus != 202) {
    http.end();
    return MediaResult::Unknown;
  }

  const int responseLength = http.getSize();
  NetworkClient* stream = http.getStreamPtr();
  if (!stream || responseLength <= 0 || responseLength > MEDIA_RESPONSE_BODY_MAX) {
    http.end();
    return MediaResult::Unknown;
  }
  char response[MEDIA_RESPONSE_BODY_MAX + 1];
  const size_t bytesRead = stream->readBytes(response, static_cast<size_t>(responseLength));
  http.end();
  if (bytesRead != static_cast<size_t>(responseLength)) return MediaResult::Unknown;
  response[bytesRead] = '\0';

  StaticJsonDocument<256> reply;
  if (deserializeJson(reply, response, bytesRead, DeserializationOption::NestingLimit(2)) ||
      !reply.is<JsonObject>() || reply.size() != 2 ||
      !reply["id"].is<const char*>() || !reply["status"].is<const char*>()) {
    return MediaResult::Unknown;
  }
  const char* responseId = reply["id"].as<const char*>();
  const char* responseState = reply["status"].as<const char*>();
  const bool idMatches = responseId && strcmp(responseId, intent.id) == 0;
  return media_classify_response(httpStatus, idMatches, responseState);
}

bool build_voice_url(char* url, size_t size, const char* path) {
  const int length = snprintf(url, size, "https://%s:%u%s", PC_MEDIA_HOST,
                              static_cast<unsigned>(PC_MEDIA_PORT), path);
  return length > 0 && static_cast<size_t>(length) < size;
}

bool send_custom_action(const MediaIntent& intent) {
  uint64_t nowUnixMs = 0;
  if (!voice_action_id_is_valid(intent.actionId) ||
      !media_request_is_fresh(intent.recognizedAtMs, millis()) ||
      !unix_time_ms(nowUnixMs) ||
      !media_wall_clock_valid(intent.issuedAtUnixMs,
                              intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS,
                              nowUnixMs) ||
      WiFi.status() != WL_CONNECTED) return false;

  StaticJsonDocument<256> request;
  request["id"] = intent.id;
  request["action_id"] = intent.actionId;
  request["voice_revision"] = intent.voiceRevision;
  request["issued_at_ms"] = intent.issuedAtUnixMs;
  request["expires_at_ms"] = intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS;
  char payload[MEDIA_REQUEST_BODY_MAX + 1];
  const size_t payloadLength = serializeJson(request, payload, sizeof(payload));
  if (payloadLength == 0 || payloadLength > MEDIA_REQUEST_BODY_MAX) return false;

  WiFiClientSecure client;
  if (!media_control_prepare_tls(client)) return false;
  HTTPClient http;
  char url[320];
  if (!build_voice_url(url, sizeof(url), "/v1/actions") || !http.begin(client, url)) return false;
  http.setReuse(false);
  http.setFollowRedirects(HTTPC_DISABLE_FOLLOW_REDIRECTS);
  http.setConnectTimeout(MEDIA_CONNECT_TIMEOUT_MS);
  http.setTimeout(MEDIA_READ_TIMEOUT_MS);
  http.setAcceptEncoding("identity");
  http.addHeader("Content-Type", "application/json");
  http.addHeader("Authorization", String("Bearer ") + PC_MEDIA_API_TOKEN);
  if (!media_request_is_fresh(intent.recognizedAtMs, millis())) {
    http.end();
    return false;
  }
  uint64_t sendTimeMs = 0;
  if (!unix_time_ms(sendTimeMs) ||
      !media_wall_clock_valid(intent.issuedAtUnixMs,
                              intent.issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS,
                              sendTimeMs)) {
    http.end();
    return false;
  }
  const int status = http.POST(reinterpret_cast<uint8_t*>(payload), payloadLength);
  if (status != 200) {
    http.end();
    printf("[voice] custom action request rejected/unknown http=%d\n", status);
    return false;
  }
  const int responseLength = http.getSize();
  NetworkClient* stream = http.getStreamPtr();
  char response[MEDIA_RESPONSE_BODY_MAX + 1];
  const size_t bytesRead = stream && responseLength > 0 && responseLength <= MEDIA_RESPONSE_BODY_MAX
                               ? stream->readBytes(response, static_cast<size_t>(responseLength))
                               : 0;
  http.end();
  if (bytesRead != static_cast<size_t>(responseLength) || responseLength <= 0) return false;
  StaticJsonDocument<256> reply;
  if (deserializeJson(reply, response, bytesRead, DeserializationOption::NestingLimit(2)) ||
      !reply.is<JsonObject>() || reply.size() != 2 ||
      !reply["id"].is<const char*>() || !reply["status"].is<const char*>() ||
      strcmp(reply["id"].as<const char*>(), intent.id) != 0 ||
      strcmp(reply["status"].as<const char*>(), "dispatched") != 0) return false;
  printf("[voice] custom action dispatched revision=%u\n", intent.voiceRevision);
  return true;
}

bool fetch_voice_config(VoiceConfigSnapshot& candidate) {
  if (WiFi.status() != WL_CONNECTED || !media_control_time_ready()) return false;
  WiFiClientSecure client;
  if (!media_control_prepare_tls(client)) return false;
  HTTPClient http;
  char url[320];
  if (!build_voice_url(url, sizeof(url), "/v1/voice-config") || !http.begin(client, url)) return false;
  http.setReuse(false);
  http.setFollowRedirects(HTTPC_DISABLE_FOLLOW_REDIRECTS);
  http.setConnectTimeout(MEDIA_CONNECT_TIMEOUT_MS);
  http.setTimeout(MEDIA_READ_TIMEOUT_MS);
  http.setAcceptEncoding("identity");
  http.addHeader("Authorization", String("Bearer ") + PC_MEDIA_API_TOKEN);
  const int status = http.GET();
  const int responseLength = http.getSize();
  NetworkClient* stream = http.getStreamPtr();
  if (status != 200 || !stream || responseLength <= 0 ||
      responseLength > static_cast<int>(VOICE_CONFIG_RESPONSE_MAX)) {
    http.end();
    printf("[voice] sync fetch rejected/unknown http=%d\n", status);
    return false;
  }
  std::unique_ptr<char[]> response(new (std::nothrow) char[VOICE_CONFIG_RESPONSE_MAX + 1]);
  if (!response) {
    http.end();
    return false;
  }
  const size_t bytesRead = stream->readBytes(response.get(), static_cast<size_t>(responseLength));
  http.end();
  if (bytesRead != static_cast<size_t>(responseLength)) return false;
  DynamicJsonDocument document(VOICE_CONFIG_RESPONSE_MAX);
  if (deserializeJson(document, response.get(), bytesRead, DeserializationOption::NestingLimit(4)) ||
      !document.is<JsonObject>() || document.size() != 4 ||
      !document["schema"].is<uint32_t>() ||
      document["schema"].as<uint32_t>() != VOICE_CONFIG_SCHEMA ||
      !document["revision"].is<uint64_t>() ||
      document["revision"].as<uint64_t>() == 0 ||
      document["revision"].as<uint64_t>() > UINT32_MAX ||
      !document["actions_revision"].is<uint64_t>() ||
      !document["commands"].is<JsonArray>()) return false;
  const JsonArray commands = document["commands"].as<JsonArray>();
  if (commands.size() > VOICE_CONFIG_MAX_COMMANDS) return false;
  candidate = {};
  candidate.schema = VOICE_CONFIG_SCHEMA;
  candidate.revision = static_cast<uint32_t>(document["revision"].as<uint64_t>());
  candidate.count = static_cast<uint16_t>(commands.size());
  uint16_t index = 0;
  for (JsonVariant value : commands) {
    if (!value.is<JsonObject>()) return false;
    const JsonObject command = value.as<JsonObject>();
    if (command.size() != 2 || !command["phrase"].is<JsonString>() ||
        !command["action_id"].is<JsonString>()) return false;
    const JsonString phrase = command["phrase"].as<JsonString>();
    const JsonString actionId = command["action_id"].as<JsonString>();
    if (phrase.size() > VOICE_PHRASE_MAX_BYTES || actionId.size() > VOICE_ACTION_ID_MAX_BYTES ||
        strlen(phrase.c_str()) != phrase.size() || strlen(actionId.c_str()) != actionId.size()) return false;
    memcpy(candidate.commands[index].phrase, phrase.c_str(), phrase.size());
    memcpy(candidate.commands[index].actionId, actionId.c_str(), actionId.size());
    ++index;
  }
  return voice_config_is_valid(candidate);
}

bool post_voice_ack(uint32_t revision) {
  StaticJsonDocument<64> request;
  request["revision"] = revision;
  char payload[64];
  const size_t length = serializeJson(request, payload, sizeof(payload));
  if (!length) return false;
  WiFiClientSecure client;
  if (!media_control_prepare_tls(client)) return false;
  HTTPClient http;
  char url[320];
  if (!build_voice_url(url, sizeof(url), "/v1/voice-config/ack") || !http.begin(client, url)) return false;
  http.setReuse(false);
  http.setFollowRedirects(HTTPC_DISABLE_FOLLOW_REDIRECTS);
  http.setConnectTimeout(MEDIA_CONNECT_TIMEOUT_MS);
  http.setTimeout(MEDIA_READ_TIMEOUT_MS);
  http.setAcceptEncoding("identity");
  http.addHeader("Content-Type", "application/json");
  http.addHeader("Authorization", String("Bearer ") + PC_MEDIA_API_TOKEN);
  const int status = http.POST(reinterpret_cast<uint8_t*>(payload), length);
  http.end();
  return status == 200;
}

void sync_voice_config() {
  VoiceConfigSnapshot candidate = {};
  if (!fetch_voice_config(candidate)) {
    puts("[voice] sync failed; keeping active settings");
    indicator_voice_sync_result(false);
    return;
  }
  if (!voice_service_apply_config(candidate)) {
    puts("[voice] sync activation failed; keeping active settings");
    indicator_voice_sync_result(false);
    return;
  }
  const bool acknowledged = post_voice_ack(candidate.revision);
  printf("[voice] sync applied revision=%u; daemon_ack=%s\n", candidate.revision,
         acknowledged ? "confirmed" : "pending");
  indicator_voice_sync_result(true);
}

void media_worker(void*) {
  MediaIntent intent;
  for (;;) {
    if (xQueueReceive(mediaQueue, &intent, portMAX_DELAY) != pdPASS) continue;
    if (intent.kind == IntentKind::SyncVoiceConfig) {
      sync_voice_config();
    } else if (intent.kind == IntentKind::CustomAction) {
      indicator_media_result(send_custom_action(intent) ? MediaResult::Executed : MediaResult::Unknown);
    } else {
      int status = 0;
      const MediaResult result = send_intent(intent, status);
      report_result(result, intent, status);
    }
  }
}
}  // namespace

void media_control_time_setup() {
  mediaConfigured = false;
  const MediaConfigStatus status = media_control_config_status();
  if (status == MediaConfigStatus::Disabled) {
    puts("[media] LAN media control disabled");
    return;
  }
  if (status != MediaConfigStatus::Ready) {
    puts("[media] invalid LAN media configuration; media control disabled");
    return;
  }

  mediaConfigured = true;
  configTime(0, 0, "pool.ntp.org", "time.nist.gov");
  puts("[media] NTP started; HTTPS remains disabled until system time is valid");
}

bool media_control_time_ready() {
  return mediaConfigured && time(nullptr) >= MIN_VALID_UNIX_TIME;
}

bool media_control_prepare_tls(WiFiClientSecure& client) {
  if (!media_control_time_ready()) return false;
  client.setCACert(PC_MEDIA_CA_CERT);
  client.setHandshakeTimeout(MEDIA_HANDSHAKE_TIMEOUT_SECONDS);
  return true;
}

void media_control_service_start() {
  if (!mediaConfigured || mediaQueue) return;
  mediaQueue = xQueueCreate(MEDIA_QUEUE_LENGTH, sizeof(MediaIntent));
  if (!mediaQueue) {
    puts("[media] request queue allocation failed; media control disabled");
    return;
  }
  if (xTaskCreate(media_worker, "media_https", MEDIA_WORKER_STACK_BYTES,
                  nullptr, 1, nullptr) != pdPASS) {
    vQueueDelete(mediaQueue);
    mediaQueue = nullptr;
    puts("[media] HTTPS worker creation failed; media control disabled");
  }
}

MediaSubmitStatus media_control_submit(MediaCommand command,
                                       uint32_t recognizedAtMs,
                                       uint64_t issuedAtUnixMs) {
  MediaIntent intent = {};
  intent.kind = IntentKind::Media;
  intent.command = command;
  intent.recognizedAtMs = recognizedAtMs;
  intent.issuedAtUnixMs = issuedAtUnixMs;

  MediaSubmitStatus result = MediaSubmitStatus::Queued;
  uint64_t nowUnixMs = 0;
  if (!mediaConfigured || !mediaQueue) {
    result = MediaSubmitStatus::Disabled;
  } else if (WiFi.status() != WL_CONNECTED) {
    result = MediaSubmitStatus::WifiUnavailable;
  } else if (!media_control_time_ready() || !unix_time_ms(nowUnixMs)) {
    result = MediaSubmitStatus::ClockNotReady;
  } else if (!media_request_is_fresh(recognizedAtMs, millis()) ||
             issuedAtUnixMs == 0 ||
             !media_wall_clock_valid(issuedAtUnixMs,
                                     issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS,
                                     nowUnixMs)) {
    result = MediaSubmitStatus::Stale;
  } else {
    make_request_id(intent.id);
    if (xQueueSend(mediaQueue, &intent, 0) != pdPASS) {
      result = MediaSubmitStatus::QueueFull;
    }
  }

  if (result == MediaSubmitStatus::Queued) {
    report_result(MediaResult::Queued, intent);
  } else {
    printf("[media] command rejected cmd=%s reason=%u\n",
           media_command_name(command), static_cast<unsigned>(result));
    indicator_media_result(MediaResult::Rejected);
  }
  return result;
}

MediaSubmitStatus media_control_submit_custom_action(const char* actionId,
                                                     uint32_t voiceRevision,
                                                     uint32_t recognizedAtMs,
                                                     uint64_t issuedAtUnixMs) {
  MediaIntent intent = {};
  intent.kind = IntentKind::CustomAction;
  intent.voiceRevision = voiceRevision;
  intent.recognizedAtMs = recognizedAtMs;
  intent.issuedAtUnixMs = issuedAtUnixMs;
  if (!voice_action_id_is_valid(actionId)) return MediaSubmitStatus::Disabled;
  snprintf(intent.actionId, sizeof(intent.actionId), "%s", actionId);
  MediaSubmitStatus result = MediaSubmitStatus::Queued;
  uint64_t nowUnixMs = 0;
  if (!mediaConfigured || !mediaQueue) {
    result = MediaSubmitStatus::Disabled;
  } else if (WiFi.status() != WL_CONNECTED) {
    result = MediaSubmitStatus::WifiUnavailable;
  } else if (!media_control_time_ready() || !unix_time_ms(nowUnixMs)) {
    result = MediaSubmitStatus::ClockNotReady;
  } else if (!media_request_is_fresh(recognizedAtMs, millis()) || issuedAtUnixMs == 0 ||
             !media_wall_clock_valid(issuedAtUnixMs, issuedAtUnixMs + MEDIA_COMMAND_LIFETIME_MS, nowUnixMs)) {
    result = MediaSubmitStatus::Stale;
  } else {
    make_request_id(intent.id);
    if (xQueueSend(mediaQueue, &intent, 0) != pdPASS) result = MediaSubmitStatus::QueueFull;
  }
  if (result == MediaSubmitStatus::Queued) {
    indicator_media_result(MediaResult::Queued);
    puts("[voice] custom action queued");
  } else {
    printf("[voice] custom action rejected reason=%u\n", static_cast<unsigned>(result));
    indicator_media_result(MediaResult::Rejected);
  }
  return result;
}

bool media_control_sync_voice_config() {
  MediaIntent intent = {};
  intent.kind = IntentKind::SyncVoiceConfig;
  if (!mediaConfigured || !mediaQueue || WiFi.status() != WL_CONNECTED ||
      !media_control_time_ready() || xQueueSend(mediaQueue, &intent, 0) != pdPASS) {
    puts("[voice] sync request unavailable or busy");
    indicator_voice_sync_result(false);
    return false;
  }
  puts("[voice] sync requested");
  return true;
}
