#include "src/services/wifi_service.h"
#include "config.h"
#include "src/services/log_service.h"
#include <WiFi.h>

static unsigned long lastReconnectAttempt = 0;
static bool wifiWasConnected = false;
static const unsigned long RECONNECT_INTERVAL_MS = 10000;

void wifi_connect() {
  WiFi.mode(WIFI_STA);
  Serial.printf("[wifi] connect ssid=%s\n", WIFI_SSID);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  uint32_t start = millis();
  uint32_t lastLog = start;
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    if (millis() - lastLog >= 5000) {
      Serial.printf("[wifi] waiting %lus status=%d\n", (millis() - start) / 1000, WiFi.status());
      lastLog = millis();
    }
    Serial.print(".");
  }

  Serial.printf("\n[wifi] connected in %lums ip=%s rssi=%d\n", millis() - start,
                WiFi.localIP().toString().c_str(), WiFi.RSSI());
  wifiWasConnected = true;
  lastReconnectAttempt = 0;
  log_event("info", "wifi", "connected", "WiFi connected");
}

void wifi_ensure() {
  if (WiFi.status() == WL_CONNECTED) {
    if (!wifiWasConnected) {
      Serial.printf("[wifi] restored uptime=%lus\n", millis() / 1000);
      log_event("info", "wifi", "restored", "WiFi restored");
    }
    wifiWasConnected = true;
    lastReconnectAttempt = 0;
    return;
  }

  if (wifiWasConnected) {
    wifiWasConnected = false;
    Serial.printf("[wifi] lost uptime=%lus\n", millis() / 1000);
    log_event("warn", "wifi", "lost", "WiFi lost");
  }

  unsigned long now = millis();
  if (lastReconnectAttempt != 0 &&
      now - lastReconnectAttempt < RECONNECT_INTERVAL_MS)
    return;
  lastReconnectAttempt = now;

  Serial.printf("[wifi] reconnect uptime=%lus\n", now / 1000);
  log_event("warn", "wifi", "reconnect", "WiFi reconnect attempt");
  WiFi.reconnect();
  delay(250);
}
