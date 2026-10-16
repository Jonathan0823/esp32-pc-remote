#include <Arduino.h>
#include <ArduinoOTA.h>
#include <Update.h>
#include <WiFi.h>
#include <esp_system.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

#include "config.h"
#include "src/services/indicator_service.h"
#include "src/services/ota_config.h"
#include "src/services/ota_policy.h"
#include "src/services/ota_service.h"

namespace {
constexpr int OTA_BOOT_GPIO = 0;
constexpr uint32_t OTA_WORKER_STACK_BYTES = 8192;
constexpr uint32_t OTA_POLL_MS = 20;
constexpr uint32_t OTA_IDLE_TIMEOUT_MS = 1000;
constexpr uint32_t OTA_TIMEOUT_RESULT_MS = 500;

OtaPolicy otaPolicy;
bool otaListenerStarted = false;
bool otaTransferStarted = false;
bool otaTerminalFailure = false;

void ota_on_start() {
  if (!otaPolicy.begin_transfer(millis(), ota_config_status() == OtaConfigStatus::Ready,
                                WiFi.status() == WL_CONNECTED,
                                ArduinoOTA.getCommand() == U_FLASH)) {
    puts("[ota] rejected non-app, stale, or unavailable transfer");
    Update.abort();
    otaTerminalFailure = true;
    return;
  }
  otaTransferStarted = true;
  indicator_ota_state(IndicatorOtaState::Transferring);
  puts("[ota] app transfer started");
}

void ota_on_progress(unsigned int progress, unsigned int total) {
  (void)progress;
  (void)total;
  if (!otaTransferStarted || !otaPolicy.transfer_expired(millis())) return;
  puts("[ota] transfer deadline exceeded; restarting before image activation");
  indicator_ota_result(false);
  vTaskDelay(pdMS_TO_TICKS(OTA_TIMEOUT_RESULT_MS));
  // Update invokes this callback inside write(); resetting avoids reentrant abort while preserving the active slot.
  esp_restart();
}

void ota_on_end() {
  otaTransferStarted = false;
  otaPolicy.finish_transfer();
  indicator_ota_result(true);
  puts("[ota] image validated; rebooting into updated app");
}

void ota_on_error(ota_error_t error) {
  otaTerminalFailure = true;
  printf("[ota] update failed category=%u\n", static_cast<unsigned>(error));
}

void start_listener() {
  otaTerminalFailure = false;
  otaTransferStarted = false;
  ArduinoOTA.setPort(3232);
  ArduinoOTA.setHostname("esp32-pc-remote");
  ArduinoOTA.setMdnsEnabled(false);
  ArduinoOTA.setPassword(PC_OTA_PASSWORD);
  ArduinoOTA.setTimeout(OTA_IDLE_TIMEOUT_MS);
  ArduinoOTA.setRebootOnSuccess(true);
  ArduinoOTA.onStart(ota_on_start);
  ArduinoOTA.onProgress(ota_on_progress);
  ArduinoOTA.onEnd(ota_on_end);
  ArduinoOTA.onError(ota_on_error);
  ArduinoOTA.begin();
  otaListenerStarted = true;
  indicator_ota_state(IndicatorOtaState::WindowOpen);
  puts("[ota] authenticated upload window open for 120 seconds");
}

void stop_listener(bool failed) {
  if (otaListenerStarted) ArduinoOTA.end();
  otaListenerStarted = false;
  otaTransferStarted = false;
  otaTerminalFailure = false;
  otaPolicy.close();
  indicator_ota_state(IndicatorOtaState::Closed);
  if (failed) indicator_ota_result(false);
}

void ota_worker(void*) {
  pinMode(OTA_BOOT_GPIO, INPUT_PULLUP);
  const uint32_t now = millis();
  OtaButtonGate button;
  button.begin(digitalRead(OTA_BOOT_GPIO) == LOW, now);

  for (;;) {
    const uint32_t current = millis();
    if (otaPolicy.state() == OtaState::WindowOpen) {
      if (WiFi.status() != WL_CONNECTED) {
        puts("[ota] Wi-Fi lost; closing upload window");
        stop_listener(true);
      } else if (otaPolicy.expire_window(current)) {
        puts("[ota] upload window expired");
        stop_listener(false);
      }
    }

    if (button.sample(digitalRead(OTA_BOOT_GPIO) == LOW, current)) {
      const OtaConfigStatus configStatus = ota_config_status();
      const bool wifiConnected = WiFi.status() == WL_CONNECTED;
      if (otaPolicy.open_window(current, configStatus == OtaConfigStatus::Ready,
                                wifiConnected)) {
        start_listener();
      } else if (configStatus != OtaConfigStatus::Ready || !wifiConnected) {
        puts("[ota] button activation rejected: configure a strong password and connect Wi-Fi");
        indicator_ota_result(false);
      }
    }

    if (otaListenerStarted && otaPolicy.state() == OtaState::WindowOpen) {
      ArduinoOTA.handle();
      if (otaTerminalFailure) stop_listener(true);
    }
    vTaskDelay(pdMS_TO_TICKS(OTA_POLL_MS));
  }
}
}  // namespace

bool ota_service_start() {
  const OtaConfigStatus status = ota_config_status();
  if (status == OtaConfigStatus::Disabled) {
    puts("[ota] disabled; set PC_OTA_PASSWORD to enable");
    return true;
  }
  if (status != OtaConfigStatus::Ready) {
    puts("[ota] invalid password configuration; OTA disabled");
    return false;
  }
  if (xTaskCreate(ota_worker, "ota", OTA_WORKER_STACK_BYTES, nullptr, 1, nullptr) != pdPASS) {
    puts("[ota] worker creation failed; OTA disabled");
    return false;
  }
  return true;
}
