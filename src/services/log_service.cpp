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

// ponytail: simple JSON string escape; no full encoder needed here
static String json_escape(const String& s) {
  String out;
  for (size_t i = 0; i < s.length(); i++) {
    char c = s[i];
    if (c == '"')       out += "\\\"";
    else if (c == '\\') out += "\\\\";
    else if (c == '\n') out += "\\n";
    else if (c == '\r') out += "\\r";
    else if (c == '\t') out += "\\t";
    else                out += c;
  }
  return out;
}

static String make_log_json(const char* level, const char* component,
                            const char* event, const char* msg) {
  String line = "{";
  line += "\"level\":\"";         line += level;      line += "\"";
  line += ",\"component\":\"";    line += component;   line += "\"";
  line += ",\"event\":\"";        line += event;       line += "\"";
  line += ",\"msg\":\"";          line += json_escape(msg); line += "\"";
  line += ",\"uptime_s\":";       line += String(millis() / 1000);
  line += ",\"heap\":";           line += String(ESP.getFreeHeap());
  if (WiFi.status() == WL_CONNECTED) {
    line += ",\"rssi\":";         line += String(WiFi.RSSI());
    line += ",\"ip\":\"";         line += WiFi.localIP().toString(); line += "\"";
  }
  line += "}";
  return line;
}

static unsigned long long log_timestamp_ns() {
  time_t now = time(nullptr);
  return (now > 100000)
    ? (unsigned long long)now * 1000000000ULL
    : 0ULL;
}

static bool send_loki_line(const String& line) {
  HTTPClient http;
  String endpoint = String(GRAFANA_LOGS_URL) + "/loki/api/v1/push";
  if (!http.begin(logClient, endpoint)) {
    Serial.println("[log] HTTP begin failed");
    return false;
  }

  http.setAuthorization(GRAFANA_LOGS_USER, GRAFANA_LOGS_TOKEN);
  http.addHeader("Content-Type", "application/json");

  String payload = "{\"streams\":[{\"stream\":{\"app\":\"esp32-pc-remote\"},\"values\":[[\"";
  payload += String(log_timestamp_ns());
  payload += "\",\"";
  payload += json_escape(line);
  payload += "\"]]}]}";

  int code = http.POST(payload);
  http.end();

  if (code < 200 || code >= 300) {
    Serial.printf("[log] Loki POST failed code=%d\n", code);
    return false;
  }
  return true;
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

  // ponytail: setInsecure for MVP; add TLS verification if logs contain sensitive data
  logClient.setInsecure();

  // Start NTP so log timestamps are accurate
  configTime(0, 0, "pool.ntp.org", "time.nist.gov");
  Serial.println("[log] init NTP + Grafana Cloud logging");

  log_event("info", "boot", "start", "ESP32 PC Remote started");
}

void log_event(const char* level, const char* component,
               const char* event, const char* msg) {
  if (!grafanaConfigured) return;
  String line = make_log_json(level, component, event, msg);
  send_loki_line(line);
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
