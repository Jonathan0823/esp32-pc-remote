#include <WiFi.h>
#include <stdarg.h>
#include <ArduinoJson.h>
#include <WiFiClientSecure.h>
#include <HTTPClient.h>
#include <time.h>
#include <esp_log.h>
#include "config.h"
#include "src/services/log_service.h"

static WiFiClientSecure logClient;
static bool grafanaConfigured = false;
static unsigned long lastHeartbeatMs = 0;
const unsigned long HEARTBEAT_MS = 60000;

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

  DynamicJsonDocument payload(1024);
  JsonArray streams = payload["streams"].to<JsonArray>();
  JsonObject streamEntry = streams.add<JsonObject>();
  JsonObject stream = streamEntry["stream"].to<JsonObject>();
  stream["app"] = "esp32-pc-remote";

  JsonArray values = streamEntry["values"].to<JsonArray>();
  JsonArray entry = values.add<JsonArray>();
  time_t now = time(nullptr);
  char ts[32];
  if (now > 100000) {
    snprintf(ts, sizeof(ts), "%llu", (unsigned long long)now * 1000000000ULL);
  } else {
    ts[0] = '\0';
  }
  entry.add(ts[0] ? ts : "0");
  entry.add(log_line);

  String payloadStr;
  serializeJson(payload, payloadStr);

  int code = http.POST((uint8_t*)payloadStr.c_str(), payloadStr.length());
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
    log_print("[log] Grafana Cloud not configured — skipping\n");
    return;
  }

  logClient.setInsecure();
  configTime(0, 0, "pool.ntp.org", "time.nist.gov");
  log_print("[log] init NTP + Grafana Cloud logging\n");
  log_event("info", "boot", "start", "ESP32 PC Remote started");
}

void log_event(const char* level, const char* component,
               const char* event, const char* msg) {
  if (!grafanaConfigured) return;

  DynamicJsonDocument lineDoc(512);
  lineDoc["level"] = level;
  lineDoc["component"] = component;
  lineDoc["event"] = event;
  lineDoc["msg"] = msg;
  lineDoc["uptime_s"] = (unsigned long)(millis() / 1000);
  lineDoc["heap"] = ESP.getFreeHeap();

  if (WiFi.status() == WL_CONNECTED) {
    lineDoc["rssi"] = WiFi.RSSI();
    lineDoc["ip"] = WiFi.localIP().toString();
  }

  String log_line;
  serializeJson(lineDoc, log_line);
  send_loki_line(log_line.c_str());
}

void log_print(const char* fmt, ...) {
  char buf[256];
  va_list args;
  va_start(args, fmt);
  vsnprintf(buf, sizeof(buf), fmt, args);
  va_end(args);

  size_t len = strlen(buf);
  if (len > 0 && buf[len - 1] == '\n') buf[len - 1] = '\0';
  ESP_LOGI("esp32-pc-remote", "%s", buf);
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();
  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}
