#include "src/services/wifi_service.h"
#include "config.h"
#include "src/services/log_service.h"
#include <WiFi.h>
#include <esp_task_wdt.h>

static unsigned long lastReconnectAttempt = 0;
static bool wifiWasConnected = false;
static const unsigned long RECONNECT_INTERVAL_MS = 10000;

static void wifi_force_ipv4_dns() {
  IPAddress dns1(1, 1, 1, 1);
  IPAddress dns2(8, 8, 8, 8);
  // ponytail: pin IPv4 DNS to dodge ESP32 core 3.x IPv6/DNS regressions.
  if (WiFi.setDNS(dns1, dns2)) {
    log_print("[wifi] DNS forced to %s / %s\n", dns1.toString().c_str(),
              dns2.toString().c_str());
  } else {
    log_print("[wifi] DNS override failed\n");
  }
}

void wifi_connect() {
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(false);
  log_print("[wifi] connect ssid=%s\n", WIFI_SSID);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  uint32_t start = millis();
  uint32_t lastLog = start;
  while (WiFi.status() != WL_CONNECTED) {
    esp_task_wdt_reset();
    delay(500);
    if (millis() - lastLog >= 5000) {
      log_print("[wifi] waiting %lus status=%d\n", (millis() - start) / 1000, WiFi.status());
      lastLog = millis();
    }
    Serial.print(".");
  }

  wifi_force_ipv4_dns();
  log_print("[wifi] connected in %lums ip=%s rssi=%d\n", millis() - start,
                WiFi.localIP().toString().c_str(), WiFi.RSSI());
  wifiWasConnected = true;
  lastReconnectAttempt = 0;
  log_event("info", "wifi", "connected", "WiFi connected");
}

void wifi_ensure() {
  if (WiFi.status() == WL_CONNECTED) {
    if (!wifiWasConnected) {
      log_print("[wifi] restored uptime=%lus\n", millis() / 1000);
      log_event("info", "wifi", "restored", "WiFi restored");
      wifi_force_ipv4_dns();
    }
    wifiWasConnected = true;
    lastReconnectAttempt = 0;
    return;
  }

  if (wifiWasConnected) {
    wifiWasConnected = false;
    log_print("[wifi] lost uptime=%lus\n", millis() / 1000);
    log_event("warn", "wifi", "lost", "WiFi lost");
  }

  unsigned long now = millis();
  if (lastReconnectAttempt != 0 &&
      now - lastReconnectAttempt < RECONNECT_INTERVAL_MS)
    return;
  lastReconnectAttempt = now;

  log_print("[wifi] reconnect uptime=%lus\n", now / 1000);
  log_event("warn", "wifi", "reconnect", "WiFi reconnect attempt");
  WiFi.reconnect();
  delay(250);
}
