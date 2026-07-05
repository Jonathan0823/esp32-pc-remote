#include <WiFi.h>
#include "config.h"
#include "src/services/wifi_service.h"

void wifi_connect() {
  WiFi.mode(WIFI_STA);
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
}

void wifi_ensure() {
  if (WiFi.status() != WL_CONNECTED) {
    Serial.printf("[wifi] lost; reconnecting uptime=%lus\n", millis() / 1000);
    WiFi.reconnect();
    delay(250);
  }
}
