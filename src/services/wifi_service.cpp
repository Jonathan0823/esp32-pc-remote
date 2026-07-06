#include <WiFi.h>
#include "config.h"
#include "src/services/wifi_service.h"
#include "src/services/log_service.h"

static unsigned long lastReconnectAttempt = 0;
static const unsigned long RECONNECT_INTERVAL_MS = 10000;

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
  log_event("info", "wifi", "connected", "WiFi connected");
}

void wifi_ensure() {
  if (WiFi.status() == WL_CONNECTED) {
    lastReconnectAttempt = 0;   // reset timer once reconnected
    return;
  }

  unsigned long now = millis();
  if (lastReconnectAttempt != 0 && now - lastReconnectAttempt < RECONNECT_INTERVAL_MS) return;
  lastReconnectAttempt = now;

  Serial.printf("[wifi] lost; restarting STA connect uptime=%lus\n", now / 1000);
  WiFi.disconnect();
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  log_event("warn", "wifi", "reconnect", "WiFi lost, reconnecting");
}
