#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <time.h>
#include <stdarg.h>
#include <stdio.h>
#include <ArduinoJson.h>
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include "config.h"
#include "src/services/log_service.h"

static bool grafanaConfigured = false;
static QueueHandle_t wakeLogQueue = nullptr;
static unsigned long lastHeartbeatMs = 0;
const unsigned long HEARTBEAT_MS = 60000;
static log_callback_t externalLogCb = NULL;

namespace {
constexpr size_t WAKE_LOG_QUEUE_LENGTH = 8;
struct WakeLogRecord {
  char source[12];
  char event[24];
  char wakeId[9];
  char reason[32];
  bool packetAttempted;
  bool sent;
  uint32_t uptimeMs;
};

void copy_log_field(char* destination, size_t size, const char* value) {
  snprintf(destination, size, "%s", value ? value : "");
}
}  // namespace

static void escape_json(const char* in, char* out, size_t out_len) {
  size_t pos = 0;
  for (; *in && pos < out_len - 6; in++) {
    char c = *in;
    switch (c) {
      case '"': out[pos++] = '\\'; out[pos++] = '"'; break;
      case '\\': out[pos++] = '\\'; out[pos++] = '\\'; break;
      case '\n': out[pos++] = '\\'; out[pos++] = 'n'; break;
      case '\r': out[pos++] = '\\'; out[pos++] = 'r'; break;
      case '\t': out[pos++] = '\\'; out[pos++] = 't'; break;
      default: out[pos++] = c; break;
    }
  }
  out[pos] = '\0';
}

static void make_ts(char* buf, size_t len) {
  time_t now = time(nullptr);
  if (now > 100000) {
    snprintf(buf, len, "%llu", (unsigned long long)now * 1000000000ULL);
  } else {
    buf[0] = '\0';
  }
}

static void send_loki_line(const char* log_line) {
  WiFiClientSecure client;
  client.setInsecure();
  HTTPClient http;
  String endpoint = String(GRAFANA_LOGS_URL) + "/loki/api/v1/push";
  if (!http.begin(client, endpoint)) return;

  // ponytail: Grafana logging is best-effort; cap wait so it can't stall the bot.
  http.setConnectTimeout(2000);
  http.setTimeout(2000);
  http.setReuse(false);
  http.setAuthorization(GRAFANA_LOGS_USER, GRAFANA_LOGS_TOKEN);
  http.addHeader("Content-Type", "application/json");

  char ts[32];
  make_ts(ts, sizeof(ts));

  char* escaped_log = (char*)malloc(1024);
  char* payload = (char*)malloc(2048);
  if (!escaped_log || !payload) {
    free(escaped_log);
    free(payload);
    return;
  }

  escape_json(log_line, escaped_log, 1024);

  int plen = snprintf(payload, 2048,
    "{\"streams\":[{\"stream\":{\"app\":\"esp32-pc-remote\"},\"values\":[[\"%s\",\"%s\"]]}]}",
    ts[0] ? ts : "0", escaped_log);

  int code = http.POST((uint8_t*)payload, plen);
  http.end();
  // ponytail: ignore HTTP errors — best-effort logging
  (void)code;
  free(escaped_log);
  free(payload);
}

void log_prepare() {
  if (wakeLogQueue == nullptr) {
    wakeLogQueue = xQueueCreate(WAKE_LOG_QUEUE_LENGTH, sizeof(WakeLogRecord));
  }
  if (wakeLogQueue == nullptr) {
    log_print("[log] wake log queue allocation failed\n");
  }

  grafanaConfigured =
    strlen(GRAFANA_LOGS_URL) > 0 &&
    strlen(GRAFANA_LOGS_USER) > 0 &&
    strlen(GRAFANA_LOGS_TOKEN) > 0;
}

void log_init() {
  log_prepare();

  if (!grafanaConfigured) {
    log_print("[log] Grafana Cloud not configured — skipping\n");
    return;
  }

  configTime(0, 0, "pool.ntp.org", "time.nist.gov");
  log_print("[log] init NTP + Grafana Cloud logging\n");
  log_event("info", "boot", "start", "ESP32 PC Remote started");
}

void log_event(const char* level, const char* component,
               const char* event, const char* msg) {
  if (!grafanaConfigured) return;

  char* escaped = (char*)malloc(1024);
  char* log_line = (char*)malloc(1536);
  if (!escaped || !log_line) {
    free(escaped);
    free(log_line);
    return;
  }
  escape_json(msg, escaped, 1024);

  String ipStr;
  int rssi = 0;
  if (WiFi.status() == WL_CONNECTED) {
    ipStr = WiFi.localIP().toString();
    rssi = WiFi.RSSI();
  }

  int needed = snprintf(log_line, 1536,
    "{\"level\":\"%s\",\"component\":\"%s\",\"event\":\"%s\",\"msg\":\"%s\","
    "\"uptime_s\":%lu,\"heap\":%u,\"rssi\":%d,\"ip\":\"%s\"}",
    level, component, event, escaped,
    (unsigned long)(millis() / 1000), ESP.getFreeHeap(),
    rssi, ipStr.c_str());

  if (needed > 0 && needed < 1536) {
    send_loki_line(log_line);
  } else {
    log_print("[log] truncated line skipped (grafana)\n");
  }

  free(escaped);
  free(log_line);
}

void log_wake_event(const char* source, const char* event, const char* wakeId,
                    const char* reason, bool packetAttempted, bool sent) {
  if (!grafanaConfigured || wakeLogQueue == nullptr) return;

  WakeLogRecord record = {};
  copy_log_field(record.source, sizeof(record.source), source);
  copy_log_field(record.event, sizeof(record.event), event);
  copy_log_field(record.wakeId, sizeof(record.wakeId), wakeId);
  copy_log_field(record.reason, sizeof(record.reason), reason);
  record.packetAttempted = packetAttempted;
  record.sent = sent;
  record.uptimeMs = millis();
  if (xQueueSend(wakeLogQueue, &record, 0) != pdPASS) {
    printf("[log] wake event dropped queue full source=%s id=%s event=%s\n",
                  record.source, record.wakeId, record.event);
  }
}

void log_process() {
  if (!grafanaConfigured || wakeLogQueue == nullptr ||
      WiFi.status() != WL_CONNECTED) return;

  WakeLogRecord record;
  if (xQueueReceive(wakeLogQueue, &record, 0) != pdPASS) return;

  DynamicJsonDocument fields(384);
  fields["source"] = record.source;
  fields["wake_id"] = record.wakeId;
  fields["target"] = PC_NAME;
  if (record.reason[0]) fields["reason"] = record.reason;
  if (record.packetAttempted) {
    fields["sent"] = record.sent;
    fields["target_mac"] = PC_MAC;
    fields["broadcast"] = WOL_BCAST;
    fields["port"] = WOL_PORT;
  }

  DynamicJsonDocument doc(768);
  doc["level"] = record.reason[0] || (record.packetAttempted && !record.sent) ? "warn" : "info";
  doc["component"] = "wake";
  doc["event"] = record.event;
  for (JsonPairConst field : fields.as<JsonObjectConst>()) {
    doc[field.key()] = field.value();
  }
  doc["uptime_ms"] = record.uptimeMs;
  doc["uptime_s"] = record.uptimeMs / 1000;
  doc["heap"] = ESP.getFreeHeap();
  doc["rssi"] = WiFi.RSSI();
  doc["ip"] = WiFi.localIP().toString();

  String line;
  serializeJson(doc, line);
  if (line.length() < 1024) send_loki_line(line.c_str());
  else log_print("[log] oversized wake event skipped\n");
}

void log_print(const char* fmt, ...) {
  char buf[512];
  va_list args;
  va_start(args, fmt);
  vsnprintf(buf, sizeof(buf), fmt, args);
  va_end(args);
  // Use the ESP-IDF console, which also carries the speech model's output.
  printf("[%lums] %s", millis(), buf);
  if (externalLogCb) {
    externalLogCb(buf);
  }
}

void log_set_callback(log_callback_t cb) {
  externalLogCb = cb;
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
