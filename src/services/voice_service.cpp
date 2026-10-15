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
#include "src/services/log_service.h"

namespace {
constexpr int MIC_BCLK_GPIO = 5;
constexpr int MIC_WS_GPIO = 6;
constexpr int MIC_DATA_GPIO = 4;
constexpr int VOICE_EVENT_QUEUE_LENGTH = 8;
constexpr uint32_t VOICE_WORKER_STACK_BYTES = 4096;
constexpr int WAKE_COMPUTER_COMMAND = 1;
static_assert(WAKE_COMPUTER_COMMAND > 0, "MultiNet command IDs must start at 1");

static const sr_cmd_t SPEECH_COMMANDS[] = {
    {WAKE_COMPUTER_COMMAND, "wake computer"},
};

enum class VoiceLed { Off, Listening, Success, WifiError };

struct SpeechEvent {
  sr_event_t event;
  int commandId;
  uint32_t detectedAt;
  char wakeId[9];
};

I2SClass microphone;
QueueHandle_t speechQueue = nullptr;
void set_rgb(uint8_t red, uint8_t green, uint8_t blue) {
  rgbLedWrite(RGB_BUILTIN, red, green, blue);
}

void set_led(VoiceLed& state, uint32_t& startedAt, VoiceLed next) {
  state = next;
  startedAt = millis();
  switch (next) {
    case VoiceLed::Listening: set_rgb(0, 0, RGB_BRIGHTNESS); break;
    case VoiceLed::Success: set_rgb(0, RGB_BRIGHTNESS, 0); break;
    case VoiceLed::WifiError: set_rgb(RGB_BRIGHTNESS, 0, 0); break;
    case VoiceLed::Off: set_rgb(0, 0, 0); break;
  }
}

void on_speech_event(sr_event_t event, int commandId, int) {
  if (speechQueue == nullptr) return;

  SpeechEvent notification = {};
  notification.event = event;
  notification.commandId = commandId;
  notification.detectedAt = millis();
  if (event == SR_EVENT_WAKEWORD) {
    const String wakeId = wake_new_id();
    snprintf(notification.wakeId, sizeof(notification.wakeId), "%s", wakeId.c_str());
  }

  if (xQueueSend(speechQueue, &notification, 0) != pdPASS) {
    if (event == SR_EVENT_WAKEWORD) {
      log_wake_event("voice", "listening_rejected", notification.wakeId,
                     "speech_event_queue_full");
    } else {
      Serial.println("[voice] speech event queue full");
    }
  }
}

void voice_worker(void*) {
  VoiceSession session = {};
  char sessionWakeId[9] = {};
  VoiceLed led = VoiceLed::Off;
  uint32_t ledStartedAt = 0;
  set_led(led, ledStartedAt, VoiceLed::Off);

  for (;;) {
    const uint32_t now = millis();
    TickType_t wait = portMAX_DELAY;
    if (led == VoiceLed::Success || led == VoiceLed::WifiError) {
      const uint32_t elapsed = now - ledStartedAt;
      if (!voice_led_pulse_active(now, ledStartedAt)) {
        set_led(led, ledStartedAt, VoiceLed::Off);
      } else {
        wait = pdMS_TO_TICKS(VOICE_LED_PULSE_MS - elapsed);
        if (wait == 0) wait = 1;
      }
    }

    SpeechEvent event;
    if (xQueueReceive(speechQueue, &event, wait) != pdPASS) {
      set_led(led, ledStartedAt, VoiceLed::Off);
      continue;
    }

    if (event.event == SR_EVENT_WAKEWORD) {
      if (session.listening) {
        log_wake_event("voice", "listening_ignored", event.wakeId,
                       "session_already_active");
        continue;
      }
      if (!voice_session_begin(session, millis())) continue;
      snprintf(sessionWakeId, sizeof(sessionWakeId), "%s", event.wakeId);
      if (!ESP_SR.setMode(SR_MODE_COMMAND)) {
        session.listening = false;
        log_wake_event("voice", "listening_rejected", sessionWakeId,
                       "command_mode_failed");
        sessionWakeId[0] = '\0';
        Serial.println("[voice] failed to enter MultiNet command mode");
        continue;
      }
      log_wake_event("voice", "listening_started", sessionWakeId);
      set_led(led, ledStartedAt, VoiceLed::Listening);
      Serial.printf("[voice] id=%s Hi ESP detected; listening for wake computer\n",
                    sessionWakeId);
      continue;
    }

    if (event.event == SR_EVENT_TIMEOUT) {
      if (session.listening) {
        session.listening = false;
        log_wake_event("voice", "listening_timeout", sessionWakeId);
        set_led(led, ledStartedAt, VoiceLed::Off);
        Serial.printf("[voice] id=%s command listening timed out\n", sessionWakeId);
      }
      if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
        if (sessionWakeId[0] != '\0') {
          log_wake_event("voice", "request_rejected", sessionWakeId,
                         "wakeword_mode_failed");
        }
        Serial.println("[voice] failed to restore wake-word mode after timeout");
      }
      sessionWakeId[0] = '\0';
      continue;
    }

    if (event.event != SR_EVENT_COMMAND) continue;
    if (!session.listening) {
      ESP_SR.setMode(SR_MODE_WAKEWORD);
      continue;
    }

    const bool accepted = event.commandId == WAKE_COMPUTER_COMMAND &&
                          voice_session_accept(session, event.detectedAt);
    if (!accepted) {
      session.listening = false;
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     event.commandId == WAKE_COMPUTER_COMMAND
                         ? "command_after_timeout"
                         : "unrecognized_command");
      set_led(led, ledStartedAt, VoiceLed::Off);
      if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
        Serial.println("[voice] failed to restore wake-word mode after rejected command");
      }
      sessionWakeId[0] = '\0';
      continue;
    }

    if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     "wakeword_mode_failed");
      set_led(led, ledStartedAt, VoiceLed::Off);
      sessionWakeId[0] = '\0';
      Serial.println("[voice] failed to restore wake-word mode after command");
      continue;
    }

    log_wake_event("voice", "request_received", sessionWakeId);
    Serial.printf("[voice] id=%s wake computer recognized; wake-word mode restored\n",
                  sessionWakeId);
    if (WiFi.status() != WL_CONNECTED) {
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     "wifi_disconnected");
      Serial.println("[voice] WoL discarded: Wi-Fi disconnected");
      set_led(led, ledStartedAt, VoiceLed::WifiError);
      sessionWakeId[0] = '\0';
      continue;
    }

    const String wakeId(sessionWakeId);
    if (!wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT, "voice", wakeId)) {
      Serial.println("[voice] WoL packet submission failed");
      set_led(led, ledStartedAt, VoiceLed::Off);
      sessionWakeId[0] = '\0';
      continue;
    }

    Serial.println("[voice] WoL packet submitted; PC startup not yet confirmed");
    set_led(led, ledStartedAt, VoiceLed::Success);
    sessionWakeId[0] = '\0';
  }
}
}  // namespace

bool voice_service_start() {
  speechQueue = xQueueCreate(VOICE_EVENT_QUEUE_LENGTH, sizeof(SpeechEvent));
  if (speechQueue == nullptr) {
    Serial.println("ERROR: voice speech-event queue allocation failed");
    return false;
  }

  if (xTaskCreate(voice_worker, "voice_wake", VOICE_WORKER_STACK_BYTES,
                  nullptr, 1, nullptr) != pdPASS) {
    Serial.println("ERROR: voice worker start failed");
    vQueueDelete(speechQueue);
    speechQueue = nullptr;
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
  if (!ESP_SR.begin(microphone, SPEECH_COMMANDS,
                    sizeof(SPEECH_COMMANDS) / sizeof(SPEECH_COMMANDS[0]),
                    SR_CHANNELS_MONO, SR_MODE_WAKEWORD, "M")) {
    Serial.println("ERROR: ESP_SR initialization failed");
    microphone.end();
    return false;
  }

  Serial.println("[voice] listening for Hi ESP");
  return true;
}
