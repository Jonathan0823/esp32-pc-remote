#include <WiFi.h>
#include "config.h"
#include "src/services/wifi_service.h"
#include "src/services/log_service.h"

static unsigned long lastReconnectAttempt = 0;
static unsigned long reconnectFailures = 0;
static bool wifiWasConnected = false;
static const unsigned long RECONNECT_INTERVAL_MS = 10000;
static const unsigned long HARD_RECOVERY_AFTER = 60000;
static const unsigned long HARD_RECOVERY_ATTEMPTS = HARD_RECOVERY_AFTER / RECONNECT_INTERVAL_MS;

void wifi_connect() {
  WiFi.mode(WIFI_STA);
  WiFi.setAutoReconnect(true);
  WiFi.persistent(false);
  Serial.printf("[wifi] connect ssid=%s\n", WIFI_SSID);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  uint32_t start = millis();
  uint32_t lastLog = start;
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    if (millis() - lastLog >= 5000) {
      Serial.printf("[wifi] waiting %lus\n", (millis() - start) / 1000);
      lastLog = millis();
    }
    Serial.print(".");
  }
  Serial.printf("\n[wifi] connected in %lums ip=%s rssi=%d\n",
                millis() - start,
                WiFi.localIP().toString().c_str(),
                WiFi.RSSI());
  wifiWasConnected = true;
  reconnectFailures = 0;
  lastReconnectAttempt = 0;
  log_event("info", "wifi", "connected", "WiFi connected");
}

static void wifi_hard_restart() {
  Serial.println("[wifi] hard restart STA stack");
  log_event("warn", "wifi", "hard_restart", "Restarting WiFi STA stack");
  WiFi.disconnect(true, true);
  WiFi.mode(WIFI_OFF);
  delay(250);
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
}

void wifi_ensure() {
  if (WiFi.status() == WL_CONNECTED) {
    if (!wifiWasConnected) {
      Serial.printf("[wifi] restored uptime=%lus\n", millis() / 1000);
      log_event("info", "wifi", "restored", "WiFi restored");
    }
    wifiWasConnected = true;
    reconnectFailures = 0;
    lastReconnectAttempt = 0;
    return;
  }

  if (wifiWasConnected) {
    wifiWasConnected = false;
    Serial.printf("[wifi] lost uptime=%lus\n", millis() / 1000);
    log_event("warn", "wifi", "lost", "WiFi lost");
  }

  unsigned long now = millis();
  if (lastReconnectAttempt != 0 && now - lastReconnectAttempt < RECONNECT_INTERVAL_MS) return;
  lastReconnectAttempt = now;
  reconnectFailures++;

  Serial.printf("[wifi] reconnect attempt=%lu uptime=%lus\n",
                reconnectFailures,
                now / 1000);
  log_event("warn", "wifi", "reconnect", "WiFi reconnect attempt");

  if (reconnectFailures >= HARD_RECOVERY_ATTEMPTS) {
    reconnectFailures = 0;
    wifi_hard_restart();
    return;
  }

  WiFi.disconnect();
  WiFi.begin(WIFI_SSID, WIFI_PASS);
}
