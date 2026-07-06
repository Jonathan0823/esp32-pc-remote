#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <time.h>
// ESP32 Arduino core provides base64.h for HTTP Basic Auth
#include <base64.h>
#include "config.h"
#include "src/services/log_service.h"

// ── Ring buffer for pending log entries ─────────────────────────────────
static const int MAX_LOG_ENTRIES = 20;
static String logBuffer[MAX_LOG_ENTRIES];
static int logHead = 0;               // next write position
static int logCount = 0;              // entries waiting to be flushed
static unsigned long lastFlushMs = 0;
static const unsigned long FLUSH_INTERVAL_MS = 30000;  // flush every 30s

static WiFiClientSecure logClient;
static bool grafanaConfigured = false;
static unsigned long lastHeartbeatMs = 0;
const unsigned long HEARTBEAT_MS = 60000;

// ── Helpers ─────────────────────────────────────────────────────────────

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
  line += "\"level\":\"";         line += level;     line += "\"";
  line += ",\"component\":\"";    line += component;  line += "\"";
  line += ",\"event\":\"";        line += event;      line += "\"";
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

// ── Initialisation ──────────────────────────────────────────────────────

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

// ── Queue a log event ───────────────────────────────────────────────────

void log_event(const char* level, const char* component,
               const char* event, const char* msg) {
  if (!grafanaConfigured) return;

  String entry = make_log_json(level, component, event, msg);
  // Drop oldest if buffer full
  if (logCount >= MAX_LOG_ENTRIES) {
    logHead = (logHead + 1) % MAX_LOG_ENTRIES;
    logCount--;
  }
  int writePos = (logHead + logCount) % MAX_LOG_ENTRIES;
  logBuffer[writePos] = entry;
  logCount++;
}

void log_heartbeat(const String& targetName) {
  if (!grafanaConfigured) return;
  if (millis() - lastHeartbeatMs < HEARTBEAT_MS) return;
  lastHeartbeatMs = millis();

  String msg = "alive target=" + targetName;
  log_event("info", "heartbeat", "tick", msg.c_str());
}

// ── Flush pending log entries to Grafana Cloud Loki ────────────────────

void log_flush() {
  if (!grafanaConfigured || logCount == 0) return;
  if (millis() - lastFlushMs < FLUSH_INTERVAL_MS) return;
  lastFlushMs = millis();

  // Collect all entries and clear the buffer atomically
  String entries[MAX_LOG_ENTRIES];
  int count = logCount;
  for (int i = 0; i < count; i++) {
    int idx = (logHead + i) % MAX_LOG_ENTRIES;
    entries[i] = logBuffer[idx];
  }
  logHead = 0;
  logCount = 0;

  // Build Loki push payload
  // Timestamp: if NTP synced use real Unix epoch ns, else use 0
  time_t now = time(nullptr);
  unsigned long long tsNs = (now > 100000)
    ? (unsigned long long)now * 1000000000ULL
    : 0ULL;

  String payload = "{\"streams\":[{\"stream\":{\"app\":\"esp32-pc-remote\"},\"values\":[";
  for (int i = 0; i < count; i++) {
    if (i > 0) payload += ",";
    payload += "[\"";
    payload += String(tsNs);
    payload += "\",\"";
    payload += json_escape(entries[i]);
    payload += "\"]";
  }
  payload += "]}]}";

  // Parse host from GRAFANA_LOGS_URL
  String url = String(GRAFANA_LOGS_URL);
  if (url.startsWith("https://")) url = url.substring(8);
  if (url.endsWith("/")) url = url.substring(0, url.length() - 1);

  if (!logClient.connect(url.c_str(), 443)) {
    Serial.println("[log] HTTPS connect failed");
    // Re-queue entries on failure
    for (int i = 0; i < count; i++) {
      int writePos = (logHead + logCount) % MAX_LOG_ENTRIES;
      logBuffer[writePos] = entries[i];
      logCount++;
      if (logCount > MAX_LOG_ENTRIES) logCount = MAX_LOG_ENTRIES;
    }
    return;
  }

  // Build Basic auth header
  String authRaw = String(GRAFANA_LOGS_USER) + ":" + String(GRAFANA_LOGS_TOKEN);
  String authB64 = base64::encode(authRaw);

  logClient.println("POST /loki/api/v1/push HTTP/1.1");
  logClient.print("Host: "); logClient.println(url);
  logClient.print("Authorization: Basic "); logClient.println(authB64);
  logClient.println("Content-Type: application/json");
  logClient.print("Content-Length: "); logClient.println(payload.length());
  logClient.println("Connection: close");
  logClient.println();
  logClient.print(payload);

  // Read HTTP response status line
  unsigned long timeout = millis() + 5000;
  while (!logClient.available() && millis() < timeout) delay(10);

  bool ok = false;
  if (logClient.available()) {
    String line = logClient.readStringUntil('\n');
    ok = line.indexOf(" 204 ") >= 0 || line.indexOf(" 200 ") >= 0;
    Serial.printf("[log] flush %d entries → %s", count, line.c_str());
  }
  if (!ok) {
    Serial.printf("[log] flush %d entries → no/unknown response\n", count);
    // Re-queue on send failure
    for (int i = 0; i < count; i++) {
      int writePos = (logHead + logCount) % MAX_LOG_ENTRIES;
      logBuffer[writePos] = entries[i];
      logCount++;
      if (logCount > MAX_LOG_ENTRIES) logCount = MAX_LOG_ENTRIES;
    }
  }
  logClient.stop();
}
