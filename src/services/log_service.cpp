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

  // ponytail: Grafana logging is best-effort; cap wait so it can't stall the bot.
  logClient.setTimeout(2000);
  http.setConnectTimeout(2000);
  http.setTimeout(2000);
  http.setReuse(false);
  http.setAuthorization(GRAFANA_LOGS_USER, GRAFANA_LOGS_TOKEN);
  http.addHeader("Content-Type", "application/json");

  char ts[32];
  make_ts(ts, sizeof(ts));

  char* escaped_log = (char*)malloc(1024);
  char* payload = (char*)malloc(2048);
  if (!escaped_log || !payload) { free(escaped_log); free(payload); return; }
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

  char* escaped = (char*)malloc(512);
  char* log_line = (char*)malloc(768);
  if (!escaped || !log_line) { free(escaped); free(log_line); return; }
  escape_json(msg, escaped, 512);

  int pos = snprintf(log_line, 768,
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
  free(escaped);
  free(log_line);
}

void log_warn(const char* component, const char* event, const char* msg) {
  log_event("warn", component, event, msg);
}

void log_error(const char* component, const char* event, const char* msg) {
  log_event("error", component, event, msg);
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
