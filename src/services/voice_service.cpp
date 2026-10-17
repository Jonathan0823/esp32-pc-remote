#include <Arduino.h>
#include <WiFi.h>
#include <stdio.h>
#include "ESP_I2S.h"
#include "ESP_SR.h"
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>
#include <sys/time.h>

#include "config.h"
#include "src/services/voice_policy.h"
#include "src/services/indicator_service.h"
#include "src/services/voice_service.h"
#include "src/services/wake_service.h"
#include "src/services/log_service.h"
#include "src/services/media_control_service.h"

namespace {
constexpr int MIC_BCLK_GPIO = 5;
constexpr int MIC_WS_GPIO = 6;
constexpr int MIC_DATA_GPIO = 4;
constexpr int VOICE_EVENT_QUEUE_LENGTH = 8;
constexpr uint32_t VOICE_WORKER_STACK_BYTES = 4096;
static_assert(VOICE_WAKE_COMMAND_ID > 0, "MultiNet command IDs must start at 1");
static_assert(MEDIA_COMMAND_FIRST_ID > VOICE_WAKE_COMMAND_ID,
              "Media command IDs must not collide with WoL");

static const sr_cmd_t SPEECH_COMMANDS[] = {
    {VOICE_WAKE_COMMAND_ID, "wake"},
    {MEDIA_COMMAND_FIRST_ID + 0, "play"},
    {MEDIA_COMMAND_FIRST_ID + 1, "pause"},
    {MEDIA_COMMAND_FIRST_ID + 2, "next"},
    {MEDIA_COMMAND_FIRST_ID + 3, "back"},
    {MEDIA_COMMAND_FIRST_ID + 4, "mute"},
    {MEDIA_COMMAND_FIRST_ID + 5, "unmute"},
};

struct SpeechEvent {
  sr_event_t event;
  int commandId;
  uint32_t detectedAt;
  uint64_t detectedAtUnixMs;
  char wakeId[9];
};

I2SClass microphone;
QueueHandle_t speechQueue = nullptr;

void on_speech_event(sr_event_t event, int commandId, int) {
  if (speechQueue == nullptr) return;

  SpeechEvent notification = {};
  notification.event = event;
  notification.commandId = commandId;
  notification.detectedAt = millis();
  if (event == SR_EVENT_COMMAND) {
    timeval now = {};
    if (gettimeofday(&now, nullptr) == 0 && now.tv_sec > 0) {
      notification.detectedAtUnixMs = static_cast<uint64_t>(now.tv_sec) * 1000ULL +
                                      now.tv_usec / 1000ULL;
    }
  }
  if (event == SR_EVENT_WAKEWORD) {
    const String wakeId = wake_new_id();
    snprintf(notification.wakeId, sizeof(notification.wakeId), "%s", wakeId.c_str());
  }

  if (xQueueSend(speechQueue, &notification, 0) != pdPASS) {
    if (event == SR_EVENT_WAKEWORD) {
      log_wake_event("voice", "listening_rejected", notification.wakeId,
                     "speech_event_queue_full");
    } else {
      puts("[voice] speech event queue full");
    }
  }
}

void voice_worker(void*) {
  VoiceSession session = {};
  char sessionWakeId[9] = {};
  for (;;) {
    SpeechEvent event;
    if (xQueueReceive(speechQueue, &event, portMAX_DELAY) != pdPASS) continue;

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
        puts("[voice] failed to enter MultiNet command mode");
        continue;
      }
      log_wake_event("voice", "listening_started", sessionWakeId);
      indicator_set_voice_listening(true);
      printf("[voice] id=%s Jarvis detected; listening for wake\n",
                    sessionWakeId);
      continue;
    }

    if (event.event == SR_EVENT_TIMEOUT) {
      if (session.listening) {
        session.listening = false;
        log_wake_event("voice", "listening_timeout", sessionWakeId);
        indicator_set_voice_listening(false);
        printf("[voice] id=%s command listening timed out\n", sessionWakeId);
      }
      if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
        if (sessionWakeId[0] != '\0') {
          log_wake_event("voice", "request_rejected", sessionWakeId,
                         "wakeword_mode_failed");
        }
        puts("[voice] failed to restore wake-word mode after timeout");
      }
      sessionWakeId[0] = '\0';
      continue;
    }

    if (event.event != SR_EVENT_COMMAND) continue;
    if (!session.listening) {
      ESP_SR.setMode(SR_MODE_WAKEWORD);
      continue;
    }

    const VoiceCommand command = voice_command_from_id(event.commandId);
    const bool isWakeCommand = command.kind == VoiceCommandKind::Wake;
    const bool isMediaCommand = command.kind == VoiceCommandKind::Media;
    const MediaCommand mediaCommand = command.media;
    const bool accepted = (isWakeCommand || isMediaCommand) &&
                          voice_session_accept(session, event.detectedAt);
    if (!accepted) {
      session.listening = false;
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     isWakeCommand || isMediaCommand
                         ? "command_after_timeout"
                         : "unrecognized_command");
      indicator_set_voice_listening(false);
      if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
        puts("[voice] failed to restore wake-word mode after rejected command");
      }
      sessionWakeId[0] = '\0';
      continue;
    }

    indicator_set_voice_listening(false);
    if (!ESP_SR.setMode(SR_MODE_WAKEWORD)) {
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     "wakeword_mode_failed");
      sessionWakeId[0] = '\0';
      puts("[voice] failed to restore wake-word mode after command");
      continue;
    }

    if (isMediaCommand) {
      const MediaSubmitStatus status = media_control_submit(
          mediaCommand, event.detectedAt, event.detectedAtUnixMs);
      printf("[voice] id=%s media command %s %s; wake-word mode restored\n",
             sessionWakeId, media_command_phrase(mediaCommand),
             status == MediaSubmitStatus::Queued ? "queued" : "rejected");
      sessionWakeId[0] = '\0';
      continue;
    }

    log_wake_event("voice", "request_received", sessionWakeId);
    printf("[voice] id=%s wake recognized; wake-word mode restored\n",
                  sessionWakeId);
    if (WiFi.status() != WL_CONNECTED) {
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     "wifi_disconnected");
      puts("[voice] WoL discarded: Wi-Fi disconnected");
      indicator_wake_result("voice", false);
      sessionWakeId[0] = '\0';
      continue;
    }

    const String wakeId(sessionWakeId);
    if (!wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT, "voice", wakeId)) {
      puts("[voice] WoL packet submission failed");
      sessionWakeId[0] = '\0';
      continue;
    }

    puts("[voice] WoL packet submitted; PC startup not yet confirmed");
    sessionWakeId[0] = '\0';
  }
}
}  // namespace

bool voice_service_start() {
  speechQueue = xQueueCreate(VOICE_EVENT_QUEUE_LENGTH, sizeof(SpeechEvent));
  if (speechQueue == nullptr) {
    puts("ERROR: voice speech-event queue allocation failed");
    return false;
  }

  if (xTaskCreate(voice_worker, "voice_wake", VOICE_WORKER_STACK_BYTES,
                  nullptr, 1, nullptr) != pdPASS) {
    puts("ERROR: voice worker start failed");
    vQueueDelete(speechQueue);
    speechQueue = nullptr;
    return false;
  }

  microphone.setPins(MIC_BCLK_GPIO, MIC_WS_GPIO, -1, MIC_DATA_GPIO);
  if (!microphone.begin(I2S_MODE_STD, 16000, I2S_DATA_BIT_WIDTH_32BIT,
                        I2S_SLOT_MODE_MONO, I2S_STD_SLOT_LEFT)) {
    puts("ERROR: I2S microphone initialization failed");
    return false;
  }

  if (!microphone.configureRX(16000, I2S_DATA_BIT_WIDTH_32BIT,
                              I2S_SLOT_MODE_MONO, I2S_RX_TRANSFORM_32_TO_16,
                              I2S_STD_SLOT_LEFT)) {
    puts("ERROR: I2S microphone receive configuration failed");
    microphone.end();
    return false;
  }

  ESP_SR.onEvent(on_speech_event);
  if (!ESP_SR.begin(microphone, SPEECH_COMMANDS,
                    sizeof(SPEECH_COMMANDS) / sizeof(SPEECH_COMMANDS[0]),
                    SR_CHANNELS_MONO, SR_MODE_WAKEWORD, "M")) {
    puts("ERROR: ESP_SR initialization failed");
    microphone.end();
    return false;
  }

  puts("[voice] listening for Jarvis");
  return true;
}
