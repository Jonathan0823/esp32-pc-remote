#include <Arduino.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <ArduinoJson.h>
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>
#include <time.h>
#include <sys/time.h>
#include <stdio.h>
#include "config.h"
#include "src/services/diagnostics_service.h"
#include "src/services/media_control_service.h"
#include "src/services/media_control_config.h"

namespace {
constexpr UBaseType_t EVENT_QUEUE_LENGTH = 16;
constexpr uint32_t WORKER_STACK_BYTES = 8192;
constexpr uint32_t CONNECT_TIMEOUT_MS = 1000;
constexpr uint32_t READ_TIMEOUT_MS = 1000;
QueueHandle_t queue = nullptr;
TaskHandle_t task = nullptr;
DiagnosticRing ring;

const char* kind_name(DiagnosticKind kind) {
  switch (kind) {
    case DiagnosticKind::ClockState: return "clock_state";
    case DiagnosticKind::PreflightRejected: return "preflight_rejected";
    case DiagnosticKind::HttpResponse: return "http_response";
    case DiagnosticKind::TlsTransport: return "tls_transport";
  }
  return "clock_state";
}
const char* category_name(DiagnosticCategory category) {
  switch (category) {
    case DiagnosticCategory::Ready: return "ready";
    case DiagnosticCategory::NotReady: return "not_ready";
    case DiagnosticCategory::WifiUnavailable: return "wifi_unavailable";
    case DiagnosticCategory::ClockUnavailable: return "clock_unavailable";
    case DiagnosticCategory::Stale: return "stale";
    case DiagnosticCategory::QueueFull: return "queue_full";
    case DiagnosticCategory::Http2xx: return "http_2xx";
    case DiagnosticCategory::Http4xx: return "http_4xx";
    case DiagnosticCategory::Http5xx: return "http_5xx";
    case DiagnosticCategory::HttpOther: return "http_other";
    case DiagnosticCategory::Dns: return "dns";
    case DiagnosticCategory::Connect: return "connect";
    case DiagnosticCategory::Certificate: return "certificate";
    case DiagnosticCategory::Handshake: return "handshake";
    case DiagnosticCategory::Unknown: return "unknown";
  }
  return "unknown";
}

void add_event(JsonArray array, const DiagnosticRecord& record) {
  JsonObject event = array.createNestedObject();
  event["schema"] = 1;
  event["uptime_ms"] = record.uptimeMs;
  if (record.hasUnixTime) event["unix_time_ms"] = record.unixTimeMs;
  else event["unix_time_ms"] = nullptr;
  event["kind"] = kind_name(record.kind);
  event["category"] = category_name(record.category);
}

bool upload_batch(const DiagnosticRecord* records, uint8_t count) {
  if (!count || WiFi.status() != WL_CONNECTED || !media_control_time_ready() ||
      media_control_config_status() != MediaConfigStatus::Ready) return false;
  WiFiClientSecure client;
  if (!media_control_prepare_tls(client)) return false;
  HTTPClient http;
  char url[320];
  int length = snprintf(url, sizeof(url), "https://%s:%u/v1/diagnostics", PC_MEDIA_HOST,
                        static_cast<unsigned>(PC_MEDIA_PORT));
  if (length <= 0 || static_cast<size_t>(length) >= sizeof(url) || !http.begin(client, url)) return false;
  http.setReuse(false);
  http.setFollowRedirects(HTTPC_DISABLE_FOLLOW_REDIRECTS);
  http.setConnectTimeout(CONNECT_TIMEOUT_MS);
  http.setTimeout(READ_TIMEOUT_MS);
  http.setAcceptEncoding("identity");
  http.addHeader("Content-Type", "application/json");
  http.addHeader("Authorization", String("Bearer ") + PC_MEDIA_API_TOKEN);
  StaticJsonDocument<2048> document;
  document["schema"] = 1;
  JsonArray events = document.createNestedArray("events");
  for (uint8_t i = 0; i < count; ++i) add_event(events, records[i]);
  char payload[2048];
  const size_t payloadSize = serializeJson(document, payload, sizeof(payload));
  if (!payloadSize || payloadSize > 2048) { http.end(); return false; }
  const int status = http.POST(reinterpret_cast<uint8_t*>(payload), payloadSize);
  http.end();
  return status == 200;
}

void worker(void*) {
  bool lastReady = false;
  uint32_t lastStateMs = 0;
  uint8_t consecutiveFailures = 0;
  uint32_t retryAtMs = 0;
  for (;;) {
    DiagnosticRecord record;
    if (xQueueReceive(queue, &record, pdMS_TO_TICKS(1000)) == pdPASS) ring.push(record);
    const uint32_t now = millis();
    const bool ready = media_control_time_ready();
    if (ready != lastReady || now - lastStateMs >= 30000) {
      uint64_t unixMs = 0;
      const time_t seconds = time(nullptr);
      const bool hasTime = seconds >= 1735689600;
      unixMs = hasTime ? static_cast<uint64_t>(seconds) * 1000ULL : 0;
      ring.push({now, unixMs, hasTime, DiagnosticKind::ClockState,
                 ready ? DiagnosticCategory::Ready : DiagnosticCategory::NotReady});
      lastReady = ready;
      lastStateMs = now;
    }
    if (ring.size() && static_cast<int32_t>(now - retryAtMs) >= 0 &&
        WiFi.status() == WL_CONNECTED && ready) {
      DiagnosticRecord batch[DIAGNOSTICS_BATCH_CAPACITY];
      const uint8_t count = ring.copy(batch, DIAGNOSTICS_BATCH_CAPACITY);
      if (upload_batch(batch, count)) {
        ring.discard(count);
        consecutiveFailures = 0;
        retryAtMs = now;
      } else {
        if (consecutiveFailures < UINT8_MAX) ++consecutiveFailures;
        retryAtMs = now + diagnostics_retry_delay_ms(consecutiveFailures - 1);
      }
    }
  }
}
}  // namespace

bool diagnostics_record(DiagnosticKind kind, DiagnosticCategory category) {
  if (!queue) return false;
  timeval now = {};
  const bool hasTime = gettimeofday(&now, nullptr) == 0 && now.tv_sec >= 1735689600;
  DiagnosticRecord record = {millis(), hasTime ? static_cast<uint64_t>(now.tv_sec) * 1000ULL + now.tv_usec / 1000ULL : 0,
                             hasTime, kind, category};
  return xQueueSend(queue, &record, 0) == pdPASS;
}

void diagnostics_service_start() {
  if (task) return;
  queue = xQueueCreate(EVENT_QUEUE_LENGTH, sizeof(DiagnosticRecord));
  if (!queue) return;
  xTaskCreate(worker, "diagnostics", WORKER_STACK_BYTES, nullptr, 0, &task);
  if (!task) { vQueueDelete(queue); queue = nullptr; }
}
