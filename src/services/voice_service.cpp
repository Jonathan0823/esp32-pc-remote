#include <Arduino.h>
#include <WiFi.h>
#include "ESP_I2S.h"
#include "ESP_SR.h"
#include "esp32-hal-rgb-led.h"
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>

#include "config.h"
#include "src/services/voice_policy.h"
#include "src/services/voice_service.h"
#include "src/services/wake_service.h"

namespace {
constexpr int MIC_BCLK_GPIO = 5;
constexpr int MIC_WS_GPIO = 6;
constexpr int MIC_DATA_GPIO = 4;
constexpr int VOICE_QUEUE_LENGTH = 1;
constexpr uint32_t VOICE_WORKER_STACK_BYTES = 4096;

struct WakeEvent {
  uint32_t detectedAt;
  bool wifiConnected;
};

I2SClass microphone;
QueueHandle_t wakeQueue = nullptr;
bool hasSuccessfulWake = false;
uint32_t lastSuccessfulWake = 0;

void set_rgb(uint8_t red, uint8_t green, uint8_t blue) {
  rgbLedWrite(RGB_BUILTIN, red, green, blue);
}

void voice_worker(void*) {
  set_rgb(0, 0, 0);
  bool ledOn = false;
  uint32_t pulseStarted = 0;

  for (;;) {
    WakeEvent event;
    TickType_t wait = portMAX_DELAY;
    if (ledOn) {
      const uint32_t elapsed = static_cast<uint32_t>(millis()) - pulseStarted;
      if (!voice_led_pulse_active(millis(), pulseStarted)) {
        set_rgb(0, 0, 0);
        ledOn = false;
        continue;
      }
      wait = pdMS_TO_TICKS(VOICE_LED_PULSE_MS - elapsed);
      if (wait == 0) wait = 1;
    }

    if (xQueueReceive(wakeQueue, &event, wait) != pdPASS) continue;

    Serial.printf("[voice] wake word detected at %lu ms\n", static_cast<unsigned long>(event.detectedAt));
    const uint32_t now = millis();
    const bool connected = event.wifiConnected && WiFi.status() == WL_CONNECTED;
    if (!connected) {
      Serial.println("[voice] WoL discarded: Wi-Fi disconnected");
      set_rgb(RGB_BRIGHTNESS, 0, 0);
      pulseStarted = now;
      ledOn = true;
      continue;
    }

    if (!voice_wake_allowed(now, connected, hasSuccessfulWake, lastSuccessfulWake)) {
      Serial.println("[voice] wake ignored: five-second cooldown");
      continue;
    }

    if (!wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT)) {
      Serial.println("[voice] WoL packet submission failed");
      continue;
    }

    lastSuccessfulWake = millis();
    hasSuccessfulWake = true;
    Serial.println("[voice] WoL packet submitted; PC startup not yet confirmed");
    set_rgb(0, RGB_BRIGHTNESS, 0);
    pulseStarted = millis();
    ledOn = true;
  }
}

void on_speech_event(sr_event_t event, int, int) {
  if (event != SR_EVENT_WAKEWORD || wakeQueue == nullptr) return;

  const WakeEvent detection = {millis(), WiFi.status() == WL_CONNECTED};
  xQueueSend(wakeQueue, &detection, 0);
}
}  // namespace

bool voice_service_start() {
  wakeQueue = xQueueCreate(VOICE_QUEUE_LENGTH, sizeof(WakeEvent));
  if (wakeQueue == nullptr) {
    Serial.println("ERROR: voice wake queue allocation failed");
    return false;
  }

  TaskHandle_t worker = nullptr;
  if (xTaskCreate(voice_worker, "voice_wake", VOICE_WORKER_STACK_BYTES,
                  nullptr, 1, &worker) != pdPASS) {
    Serial.println("ERROR: voice wake worker start failed");
    vQueueDelete(wakeQueue);
    wakeQueue = nullptr;
    return false;
  }

  microphone.setPins(MIC_BCLK_GPIO, MIC_WS_GPIO, -1, MIC_DATA_GPIO);
  if (!microphone.begin(I2S_MODE_STD, 16000, I2S_DATA_BIT_WIDTH_32BIT,
                        I2S_SLOT_MODE_MONO, I2S_STD_SLOT_LEFT)) {
    Serial.println("ERROR: I2S microphone initialization failed");
    return false;
  }

  if (!microphone.configureRX(16000, I2S_DATA_BIT_WIDTH_32BIT,
                              I2S_SLOT_MODE_MONO, I2S_RX_TRANSFORM_32_TO_16,
                              I2S_STD_SLOT_LEFT)) {
    Serial.println("ERROR: I2S microphone receive configuration failed");
    microphone.end();
    return false;
  }

  ESP_SR.onEvent(on_speech_event);
  if (!ESP_SR.begin(microphone, nullptr, 0, SR_CHANNELS_MONO,
                    SR_MODE_WAKEWORD, "M")) {
    Serial.println("ERROR: ESP_SR initialization failed");
    microphone.end();
    return false;
  }

  Serial.println("[voice] listening for Hi ESP");
  return true;
}
