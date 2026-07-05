/*
 * ESP32 Telegram Bot — suibot-home-tele-esp
 *
 * Dependencies (install via Arduino CLI):
 *   arduino-cli core install esp32:esp32
 *   arduino-cli lib install "UniversalTelegramBot"
 *
 * Setup:
 *   1. Copy config.example.h → config.h
 *   2. Fill in WIFI_SSID, WIFI_PASS, BOT_TOKEN
 *   3. Compile & upload:
 *      arduino-cli compile --fqbn esp32:esp32:esp32 .
 *      arduino-cli upload --fqbn esp32:esp32:esp32 -p /dev/ttyUSB0 .
 */

#include <WiFi.h>
#include "config.h"
#include "src/services/wifi_service.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"

void setup() {
  Serial.begin(115200);
  Serial.println("\n=== ESP32 Telegram Bot ===");
  wifi_connect();
  telegram_setup();
}

void loop() {
  wifi_ensure();
  if (WiFi.status() == WL_CONNECTED) {
    telegram_poll();
  }
}
