#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <time.h>
#include "config.h"
#include "src/services/log_service.h"

static WiFiClientSecure logClient;
static bool grafanaConfigured = false;
static unsigned long lastHeartbeatMs = 0;
const unsigned long HEARTBEAT_MS = 60000;

static void escape_json(const char* in, char* out, size_t out_len) {
  size_t pos = 0;
  for (; *in && pos < out_len - 6; in++) {
    char c = *in;
    switch (c) {
      case '"':  out[pos++] = '\\'; out[pos++] = '"'; break;
      case '\\': out[pos++] = '\\'; out[pos++] = '\\'; break;
      case '\n': out[pos++] = '\\'; out[pos++] = 'n'; break;
      case '\r': out[pos++] = '\\'; out[pos++] = 'r'; break;
      case '\t': out[pos++] = '\\'; out[pos++] = 't'; break;
      default:   out[pos++] = c;
    }
  }
  out[pos] = '\0';
}

static void make_ts(char* buf, size_t len) {
  time_t now = time(nullptr);
  if (now > 100000)
    snprintf(buf, len, "%llu", (unsigned long long)now * 1000000000ULL);
  else
    buf[0] = '\0';
}

static void send_loki_line(const char* log_line) {
  HTTPClient http;
  String endpoint = String(GRAFANA_LOGS_URL) + "/loki/api/v1/push";
  if (!http.begin(logClient, endpoint)) return;

  http.setAuthorization(GRAFANA_LOGS_USER, GRAFANA_LOGS_TOKEN);
  http.addHeader("Content-Type", "application/json");

  char ts[32];
  make_ts(ts, sizeof(ts));

  char escaped_log[1024];
  escape_json(log_line, escaped_log, sizeof(escaped_log));

  char payload[2048];
  int plen = snprintf(payload, sizeof(payload),
    "{\"streams\":[{\"stream\":{\"app\":\"esp32-pc-remote\"},\"values\":[[\"%s\",\"%s\"]]}]}",
    ts[0] ? ts : "0", escaped_log);

  int code = http.POST((uint8_t*)payload, plen);
  http.end();
  // ponytail: ignore HTTP errors — best-effort logging
  (void)code;
}

void log_init() {
  grafanaConfigured =
    strlen(GRAFANA_LOGS_URL) > 0 &&
    strlen(GRAFANA_LOGS_USER) > 0 &&
    strlen(GRAFANA_LOGS_TOKEN) > 0;

  if (!grafanaConfigured) {
    Serial.println("[log] Grafana Cloud not configured — skipping");
    return;
  }

  logClient.setInsecure();
  configTime(0, 0, "pool.ntp.org", "time.nist.gov");
  Serial.println("[log] init NTP + Grafana Cloud logging");
  log_event("info", "boot", "start", "ESP32 PC Remote started");
}

void log_event(const char* level, const char* component,
               const char* event, const char* msg) {
  if (!grafanaConfigured) return;

  char escaped[512];
  escape_json(msg, escaped, sizeof(escaped));

  char log_line[768];
  int pos = snprintf(log_line, sizeof(log_line),
    "{\"level\":\"%s\",\"component\":\"%s\",\"event\":\"%s\",\"msg\":\"%s\",\"uptime_s\":%lu,\"heap\":%u",
    level, component, event, escaped,
    (unsigned long)(millis() / 1000), ESP.getFreeHeap());

  if (WiFi.status() == WL_CONNECTED) {
    snprintf(log_line + pos, sizeof(log_line) - pos,
      ",\"rssi\":%d,\"ip\":\"%s\"}",
      WiFi.RSSI(), WiFi.localIP().toString().c_str());
  } else {
    snprintf(log_line + pos, sizeof(log_line) - pos, "}");
  }

  send_loki_line(log_line);
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
