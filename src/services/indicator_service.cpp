#include <Arduino.h>
#include <stdio.h>
#include "esp32-hal-rgb-led.h"
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>

#include "config.h"
#include "src/services/indicator_policy.h"
#include "src/services/indicator_service.h"

namespace {
// shortcut: bursts beyond four queued indications drop LED feedback; coalesce if that becomes common.
constexpr int INDICATOR_QUEUE_LENGTH = 4;
constexpr uint32_t INDICATOR_WORKER_STACK_BYTES = 2048;

enum class IndicatorEventType { Listening, WakeResult, MediaFeedback };

struct IndicatorEvent {
  IndicatorEventType type;
  bool active;
  bool sent;
  MediaResult mediaResult;
  char source[12];
};

QueueHandle_t indicatorQueue = nullptr;

void show(IndicatorColor color) {
  rgbLedWrite(RGB_BUILTIN, color.red, color.green, color.blue);
}

void indicator_worker(void*) {
  bool voiceListening = false;
  bool pulseActive = false;
  uint32_t pulseStartedAt = 0;
  IndicatorColor pulseColor = {0, 0, 0};
  show(pulseColor);

  for (;;) {
    TickType_t wait = portMAX_DELAY;
    if (pulseActive) {
      const uint32_t now = millis();
      const uint32_t elapsed = now - pulseStartedAt;
      if (!indicator_pulse_active(now, pulseStartedAt)) {
        pulseActive = false;
        show(voiceListening ? indicator_listening_color(RGB_BRIGHTNESS)
                            : IndicatorColor{0, 0, 0});
      } else {
        wait = pdMS_TO_TICKS(INDICATOR_PULSE_MS - elapsed);
        if (wait == 0) wait = 1;
      }
    }

    IndicatorEvent event;
    if (xQueueReceive(indicatorQueue, &event, wait) != pdPASS) continue;

    if (event.type == IndicatorEventType::Listening) {
      voiceListening = event.active;
      if (!pulseActive) {
        show(voiceListening ? indicator_listening_color(RGB_BRIGHTNESS)
                            : IndicatorColor{0, 0, 0});
      }
    } else {
      pulseColor = event.type == IndicatorEventType::WakeResult
                       ? indicator_wake_color(event.source, event.sent, RGB_BRIGHTNESS)
                       : indicator_media_color(event.mediaResult, RGB_BRIGHTNESS);
      pulseStartedAt = millis();
      pulseActive = true;
      show(pulseColor);
    }
  }
}

void send_indicator_event(const IndicatorEvent& event) {
  if (indicatorQueue && xQueueSend(indicatorQueue, &event, 0) != pdPASS) {
    printf("[indicator] event dropped: queue full\n");
  }
}
}  // namespace

bool indicator_service_start() {
  if (indicatorQueue) return true;
  indicatorQueue = xQueueCreate(INDICATOR_QUEUE_LENGTH, sizeof(IndicatorEvent));
  if (!indicatorQueue) {
    printf("[indicator] queue allocation failed\n");
    return false;
  }
  if (xTaskCreate(indicator_worker, "indicator", INDICATOR_WORKER_STACK_BYTES,
                  nullptr, 1, nullptr) != pdPASS) {
    vQueueDelete(indicatorQueue);
    indicatorQueue = nullptr;
    printf("[indicator] worker creation failed\n");
    return false;
  }
  return true;
}

void indicator_set_voice_listening(bool listening) {
  IndicatorEvent event = {};
  event.type = IndicatorEventType::Listening;
  event.active = listening;
  send_indicator_event(event);
}

void indicator_wake_result(const char* source, bool sent) {
  IndicatorEvent event = {};
  event.type = IndicatorEventType::WakeResult;
  event.sent = sent;
  snprintf(event.source, sizeof(event.source), "%s", source ? source : "");
  send_indicator_event(event);
}

void indicator_media_result(MediaResult result) {
  IndicatorEvent event = {};
  event.type = IndicatorEventType::MediaFeedback;
  event.mediaResult = result;
  send_indicator_event(event);
}
