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
#include <esp_system.h>
#include "config.h"
#include "src/services/wifi_service.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"
#include "src/services/device_service.h"

static const char* reset_reason_label(esp_reset_reason_t reason) {
  switch (reason) {
    case ESP_RST_POWERON: return "poweron";
    case ESP_RST_EXT: return "external";
    case ESP_RST_SW: return "software";
    case ESP_RST_PANIC: return "panic";
    case ESP_RST_INT_WDT: return "int_wdt";
    case ESP_RST_TASK_WDT: return "task_wdt";
    case ESP_RST_WDT: return "other_wdt";
    case ESP_RST_DEEPSLEEP: return "deepsleep";
    case ESP_RST_BROWNOUT: return "brownout";
    case ESP_RST_SDIO: return "sdio";
    default: return "unknown";
  }
}

void setup() {
  Serial.begin(115200);
  delay(50);
  Serial.println("\n=== ESP32 Telegram Bot ===");
  Serial.printf("[boot] reset=%s heap=%u\n", reset_reason_label(esp_reset_reason()), ESP.getFreeHeap());
  device_init();
  Serial.printf("[boot] target=%s\n", device_active_name().c_str());
  wifi_connect();
  telegram_setup();
}

void loop() {
  wifi_ensure();
  if (WiFi.status() == WL_CONNECTED) {
    telegram_poll();
  }
}
