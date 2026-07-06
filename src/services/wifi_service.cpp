#include "src/services/wifi_service.h"
#include "config.h"
#include "src/services/log_service.h"
#include <WiFi.h>

static unsigned long lastReconnectAttempt = 0;
static bool wifiWasConnected = false;
static bool wifiEventRegistered = false;
static const unsigned long RECONNECT_INTERVAL_MS = 10000;

static void wifi_disconnected(WiFiEvent_t, WiFiEventInfo_t info) {
  auto reason = (wifi_err_reason_t)info.wifi_sta_disconnected.reason;
  Serial.printf("[wifi] disconnected reason=%u %s\n", reason,
                WiFi.disconnectReasonName(reason));
}

static bool wifi_preferred_auth(wifi_auth_mode_t auth) {
  return auth == WIFI_AUTH_WPA2_PSK || auth == WIFI_AUTH_WPA_WPA2_PSK ||
         auth == WIFI_AUTH_WPA_PSK;
}

static bool wifi_begin_best_ap() {
  int best = -1;
  int fallback = -1;
  int16_t count = WiFi.scanNetworks(false, false, false, 300, 0, WIFI_SSID);
  Serial.printf("[wifi] scan found=%d\n", count);

  for (int i = 0; i < count; i++) {
    wifi_auth_mode_t auth = WiFi.encryptionType(i);
    Serial.printf("[wifi] ap bssid=%s channel=%ld rssi=%ld auth=%d\n",
                  WiFi.BSSIDstr(i).c_str(), WiFi.channel(i), WiFi.RSSI(i), auth);

    if (fallback < 0 || WiFi.RSSI(i) > WiFi.RSSI(fallback)) fallback = i;
    if (wifi_preferred_auth(auth) &&
        (best < 0 || WiFi.RSSI(i) > WiFi.RSSI(best))) best = i;
  }

  if (best < 0) best = fallback;
  if (best < 0) return false;

  uint8_t bssid[6];
  WiFi.BSSID(best, bssid);
  int32_t channel = WiFi.channel(best);
  Serial.printf("[wifi] using bssid=%s channel=%ld rssi=%ld auth=%d\n",
                WiFi.BSSIDstr(best).c_str(), channel, WiFi.RSSI(best),
                WiFi.encryptionType(best));
  WiFi.begin(WIFI_SSID, WIFI_PASS, channel, bssid);
  WiFi.scanDelete();
  return true;
}

void wifi_connect() {
  if (!wifiEventRegistered) {
    WiFi.onEvent(wifi_disconnected, ARDUINO_EVENT_WIFI_STA_DISCONNECTED);
    wifiEventRegistered = true;
  }

  WiFi.disconnect(true, true);
  delay(1000);
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(false);
  WiFi.setTxPower(WIFI_POWER_19_5dBm);
  WiFi.setMinSecurity(WIFI_AUTH_WPA_PSK);
  Serial.printf("[wifi] connect ssid=%s\n", WIFI_SSID);
  if (!wifi_begin_best_ap()) WiFi.begin(WIFI_SSID, WIFI_PASS);
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
  if (!wifi_begin_best_ap()) WiFi.reconnect();
  delay(250);
}
