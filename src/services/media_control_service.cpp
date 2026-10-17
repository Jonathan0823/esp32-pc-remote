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
#include <sys/time.h>
#include <time.h>

#include "config.h"
#include "src/services/media_control_config.h"
#include "src/services/media_control_policy.h"
#include "src/services/media_control_service.h"
#include "src/services/indicator_service.h"

namespace {
constexpr time_t MIN_VALID_UNIX_TIME = 1735689600;  // 2025-01-01 UTC
constexpr UBaseType_t MEDIA_QUEUE_LENGTH = 1;
constexpr uint32_t MEDIA_WORKER_STACK_BYTES = 12288;
constexpr uint32_t MEDIA_CONNECT_TIMEOUT_MS = 1000;
constexpr uint32_t MEDIA_READ_TIMEOUT_MS = 1000;
constexpr unsigned long MEDIA_HANDSHAKE_TIMEOUT_SECONDS = 2;

struct MediaIntent {
  MediaCommand command;
  uint32_t recognizedAtMs;
  uint64_t issuedAtUnixMs;
  char id[33];
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

void media_worker(void*) {
  MediaIntent intent;
  for (;;) {
    if (xQueueReceive(mediaQueue, &intent, portMAX_DELAY) != pdPASS) continue;
    int status = 0;
    const MediaResult result = send_intent(intent, status);
    report_result(result, intent, status);
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
