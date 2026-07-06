/*
 * ESP32 Telegram Bot — esp32-pc-remote
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
#include <esp_task_wdt.h>
#include "config.h"
#include "src/services/wifi_service.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"
#include "src/services/log_service.h"

const char* reset_reason_label(esp_reset_reason_t reason) {
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

const char* current_reset_reason() {
  return reset_reason_label(esp_reset_reason());
}

void setup() {
  Serial.begin(115200);
  delay(50);
  Serial.println("\n=== ESP32 PC Remote ===");
  Serial.printf("[boot] reset=%s heap=%u\n", current_reset_reason(), ESP.getFreeHeap());
  Serial.printf("[boot] target=%s\n", PC_NAME);
  wifi_connect();
  telegram_setup();
  log_init();

  // log once if previous run was watchdog-triggered (WiFi is up after wifi_connect)
  if (esp_reset_reason() == ESP_RST_TASK_WDT) {
    log_event("warn", "wdt", "triggered", "Task watchdog triggered reset");
  }

  esp_err_t wdtStatus = esp_task_wdt_status(NULL);
  if (wdtStatus == ESP_ERR_INVALID_STATE) {
    // ponytail: Telegram long-poll blocks the loop task for ~60s
    esp_task_wdt_config_t wdtConfig = {120000, 0, true};
    esp_err_t err = esp_task_wdt_init(&wdtConfig);
    Serial.printf("[wdt] init err=%d\n", err);
    wdtStatus = err;
  } else if (wdtStatus == ESP_OK) {
    Serial.println("[wdt] current task already subscribed");
  } else {
    Serial.printf("[wdt] status err=%d\n", wdtStatus);
  }

  if (wdtStatus == ESP_ERR_NOT_FOUND) {
    esp_err_t err = esp_task_wdt_add(NULL);
    Serial.printf("[wdt] add err=%d\n", err);
  }
}

void loop() {
  wifi_ensure();
  log_heartbeat(PC_NAME);

  if (WiFi.status() == WL_CONNECTED) {
    wake_poll();
    telegram_poll();
  }

  esp_task_wdt_reset();
}
