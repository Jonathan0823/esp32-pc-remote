#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <time.h>
#include <ArduinoJson.h>
#include "config.h"
#include "src/services/log_service.h"

static WiFiClientSecure logClient;
static bool grafanaConfigured = false;
static unsigned long lastHeartbeatMs = 0;
const unsigned long HEARTBEAT_MS = 60000;

static void make_ts(char* buf, size_t len) {
  time_t now = time(nullptr);
  if (now > 100000)
    snprintf(buf, len, "%llu", (unsigned long long)now * 1000000000ULL);
  else
    buf[0] = '\0';
}

static void send_loki_line(const char* logLine) {
  HTTPClient http;
  String endpoint = String(GRAFANA_LOGS_URL) + "/loki/api/v1/push";
  if (!http.begin(logClient, endpoint)) return;

  http.setAuthorization(GRAFANA_LOGS_USER, GRAFANA_LOGS_TOKEN);
  http.addHeader("Content-Type", "application/json");

  char ts[32];
  make_ts(ts, sizeof(ts));

  DynamicJsonDocument envelope(1024);
  JsonArray streams = envelope.createNestedArray("streams");
  JsonObject s = streams.createNestedObject();
  JsonObject labels = s.createNestedObject("stream");
  labels["app"] = "esp32-pc-remote";
  JsonArray values = s.createNestedArray("values");
  JsonArray pair = values.createNestedArray();
  pair.add(ts[0] ? ts : "0");
  pair.add(logLine);

  String payload;
  serializeJson(envelope, payload);
  http.POST((uint8_t*)payload.c_str(), payload.length());
  http.end();
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

  DynamicJsonDocument doc(512);
  doc["level"] = level;
  doc["component"] = component;
  doc["event"] = event;
  doc["msg"] = msg;
  doc["uptime_s"] = (unsigned long)(millis() / 1000);
  doc["heap"] = ESP.getFreeHeap();
  if (WiFi.status() == WL_CONNECTED) {
    doc["rssi"] = WiFi.RSSI();
    doc["ip"] = WiFi.localIP().toString();
  }

  String logLine;
  serializeJson(doc, logLine);
  send_loki_line(logLine.c_str());
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
