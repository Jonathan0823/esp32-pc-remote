#include <Arduino.h>
#include <WiFi.h>
#include <Preferences.h>
#include <atomic>
#include <stdio.h>
#include <sys/time.h>
#include "ESP_I2S.h"
#include "ESP_SR.h"
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>
#include <freertos/task.h>
#include "esp_mn_speech_commands.h"
#include "model_path.h"

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
constexpr char VOICE_PREFS_NAMESPACE[] = "voice_cfg";
static_assert(VOICE_WAKE_COMMAND_ID > 0, "MultiNet command IDs must start at 1");
static_assert(MEDIA_COMMAND_FIRST_ID > VOICE_WAKE_COMMAND_ID,
              "Media command IDs must not collide with WoL");

struct BuiltinSpeechCommand { int id; const char* phrase; };
static const BuiltinSpeechCommand BUILTIN_COMMANDS[] = {
    {VOICE_WAKE_COMMAND_ID, "wake"},
    {MEDIA_COMMAND_FIRST_ID + 0, "play"},
    {MEDIA_COMMAND_FIRST_ID + 1, "pause"},
    {MEDIA_COMMAND_FIRST_ID + 2, "next"},
    {MEDIA_COMMAND_FIRST_ID + 3, "back"},
    {MEDIA_COMMAND_FIRST_ID + 4, "mute"},
    {MEDIA_COMMAND_FIRST_ID + 5, "unmute"},
    {VOICE_SYNC_COMMAND_ID, "sync"},
};

struct SpeechEvent {
  sr_event_t event;
  int commandId;
  uint32_t detectedAt;
  uint64_t detectedAtUnixMs;
  uint32_t voiceRevision;
  char wakeId[9];
};

struct VoiceConfigUpdate {
  VoiceConfigSnapshot config;
  TaskHandle_t requester;
  bool success;
};

enum class WorkerMessageKind : uint8_t { Speech, ConfigUpdate };
struct VoiceWorkerMessage {
  WorkerMessageKind kind;
  SpeechEvent speech;
  VoiceConfigUpdate* update;
};

I2SClass microphone;
Preferences voicePrefs;
QueueHandle_t speechQueue = nullptr;
VoiceConfigSnapshot activeConfig = {};
std::atomic<uint32_t> callbackVoiceRevision{0};
bool voicePrefsReady = false;

void on_speech_event(sr_event_t event, int commandId, int);

void release_model_reference() {
  srmodel_list_t* models = get_static_srmodels();
  if (models) esp_srmodel_deinit_with_refcount(models);
}

bool read_config_slot(uint8_t slot, VoiceConfigSnapshot& config) {
  const char* key = slot == 0 ? "voice0" : "voice1";
  if (voicePrefs.getBytesLength(key) != sizeof(config) ||
      voicePrefs.getBytes(key, &config, sizeof(config)) != sizeof(config)) return false;
  return voice_config_is_valid(config);
}

bool build_speech_commands(const VoiceConfigSnapshot& config, sr_cmd_t* commands,
                           size_t& count) {
  if (config.revision != 0 && !voice_config_is_valid(config)) return false;
  count = 0;
  for (const BuiltinSpeechCommand& builtin : BUILTIN_COMMANDS) {
    commands[count].command_id = builtin.id;
    snprintf(commands[count].str, sizeof(commands[count].str), "%s", builtin.phrase);
    ++count;
  }
  for (uint16_t index = 0; index < config.count; ++index) {
    commands[count].command_id = VOICE_CUSTOM_COMMAND_FIRST_ID + index;
    snprintf(commands[count].str, sizeof(commands[count].str), "%s", config.commands[index].phrase);
    voice_phrase_normalize(commands[count].str);
    ++count;
  }
  return true;
}

bool start_recognizer(const VoiceConfigSnapshot& config) {
  if (config.revision != 0 && !voice_config_is_valid(config)) return false;
  const size_t count = sizeof(BUILTIN_COMMANDS) / sizeof(BUILTIN_COMMANDS[0]) + config.count;
  sr_cmd_t* commands = static_cast<sr_cmd_t*>(
      heap_caps_calloc(count, sizeof(sr_cmd_t), MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
  if (!commands) commands = static_cast<sr_cmd_t*>(calloc(count, sizeof(sr_cmd_t)));
  size_t filled = 0;
  if (!commands || !build_speech_commands(config, commands, filled) ||
      !ESP_SR.begin(microphone, commands, filled, SR_CHANNELS_MONO,
                    SR_MODE_WAKEWORD, "M")) {
    heap_caps_free(commands);
    release_model_reference();
    return false;
  }
  // ESP_SR.begin logs MultiNet phrase errors but does not propagate them.
  bool valid = true;
  for (size_t i = 0; valid && i < filled; ++i) {
    char* phrase = esp_mn_commands_get_string(commands[i].command_id);
    valid = phrase && strcmp(phrase, commands[i].str) == 0;
  }
  heap_caps_free(commands);
  if (!valid) {
    puts("[voice] recognizer rejected a configured phrase/table");
    if (ESP_SR.end()) release_model_reference();
    return false;
  }
  return true;
}

bool same_voice_config(const VoiceConfigSnapshot& left, const VoiceConfigSnapshot& right) {
  if (left.schema != right.schema || left.revision != right.revision || left.count != right.count) return false;
  for (uint16_t i = 0; i < left.count; ++i) {
    if (strcmp(left.commands[i].phrase, right.commands[i].phrase) != 0 ||
        strcmp(left.commands[i].actionId, right.commands[i].actionId) != 0) return false;
  }
  return true;
}

bool activate_config(const VoiceConfigSnapshot& candidate) {
  if (!voicePrefsReady || !voice_config_is_valid(candidate)) return false;
  if (candidate.revision == activeConfig.revision) return same_voice_config(candidate, activeConfig);
  if (candidate.revision < activeConfig.revision) return false;

  const uint8_t currentSlot = voicePrefs.getUChar("slot", 0);
  if (currentSlot > 1) return false;
  const uint8_t candidateSlot = static_cast<uint8_t>(1 - currentSlot);
  const char* key = candidateSlot == 0 ? "voice0" : "voice1";
  if (voicePrefs.putBytes(key, &candidate, sizeof(candidate)) != sizeof(candidate)) return false;
  VoiceConfigSnapshot persisted = {};
  if (!read_config_slot(candidateSlot, persisted) || !same_voice_config(candidate, persisted)) return false;

  if (!ESP_SR.end()) return false;
  release_model_reference();
  xQueueReset(speechQueue);
  if (!start_recognizer(candidate)) {
    if (!start_recognizer(activeConfig)) {
      VoiceConfigSnapshot builtins = {};
      if (start_recognizer(builtins)) {
        activeConfig = builtins;
        callbackVoiceRevision.store(0, std::memory_order_relaxed);
      }
    }
    return false;
  }
  if (voicePrefs.putUChar("slot", candidateSlot) != 1) {
    if (ESP_SR.end()) release_model_reference();
    if (!start_recognizer(activeConfig)) {
      VoiceConfigSnapshot builtins = {};
      if (start_recognizer(builtins)) {
        activeConfig = builtins;
        callbackVoiceRevision.store(0, std::memory_order_relaxed);
      }
    }
    return false;
  }
  activeConfig = candidate;
  callbackVoiceRevision.store(activeConfig.revision, std::memory_order_relaxed);
  return true;
}

void finish_config_update(VoiceConfigUpdate* update) {
  update->success = activate_config(update->config);
  xTaskNotifyGive(update->requester);
}

void on_speech_event(sr_event_t event, int commandId, int) {
  if (speechQueue == nullptr) return;
  VoiceWorkerMessage message = {};
  message.kind = WorkerMessageKind::Speech;
  SpeechEvent& notification = message.speech;
  notification.event = event;
  notification.commandId = commandId;
  notification.detectedAt = millis();
  notification.voiceRevision = callbackVoiceRevision.load(std::memory_order_relaxed);
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
  if (xQueueSend(speechQueue, &message, 0) != pdPASS) {
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
  VoiceConfigUpdate* pendingUpdate = nullptr;
  for (;;) {
    if (pendingUpdate && !session.listening) {
      VoiceConfigUpdate* update = pendingUpdate;
      pendingUpdate = nullptr;
      finish_config_update(update);
    }

    VoiceWorkerMessage message = {};
    if (xQueueReceive(speechQueue, &message, portMAX_DELAY) != pdPASS) continue;
    if (message.kind == WorkerMessageKind::ConfigUpdate) {
      if (session.listening) {
        pendingUpdate = message.update;
      } else {
        finish_config_update(message.update);
      }
      continue;
    }

    SpeechEvent& event = message.speech;
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
      printf("[voice] id=%s Jarvis detected; listening for a command\n", sessionWakeId);
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
    const bool isSyncCommand = command.kind == VoiceCommandKind::Sync;
    const bool isCustomCommand = command.kind == VoiceCommandKind::Custom;
    const bool knownCommand = isWakeCommand || isMediaCommand || isSyncCommand || isCustomCommand;
    const bool accepted = knownCommand && voice_session_accept(session, event.detectedAt);
    if (!accepted) {
      session.listening = false;
      log_wake_event("voice", "request_rejected", sessionWakeId,
                     knownCommand ? "command_after_timeout" : "unrecognized_command");
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
          command.media, event.detectedAt, event.detectedAtUnixMs);
      printf("[voice] id=%s media command %s %s; wake-word mode restored\n",
             sessionWakeId, media_command_phrase(command.media),
             status == MediaSubmitStatus::Queued ? "queued" : "rejected");
      sessionWakeId[0] = '\0';
      continue;
    }

    if (isSyncCommand) {
      const bool queued = media_control_sync_voice_config();
      printf("[voice] id=%s sync %s\n", sessionWakeId, queued ? "queued" : "rejected");
      sessionWakeId[0] = '\0';
      continue;
    }

    if (isCustomCommand) {
      if (event.voiceRevision != activeConfig.revision || command.customIndex >= activeConfig.count) {
        puts("[voice] stale custom command ignored");
        sessionWakeId[0] = '\0';
        continue;
      }
      const VoiceConfigCommand& custom = activeConfig.commands[command.customIndex];
      const MediaSubmitStatus status = media_control_submit_custom_action(
          custom.actionId, event.voiceRevision, event.detectedAt, event.detectedAtUnixMs);
      printf("[voice] id=%s custom action %s\n", sessionWakeId,
             status == MediaSubmitStatus::Queued ? "queued" : "rejected");
      sessionWakeId[0] = '\0';
      continue;
    }

    log_wake_event("voice", "request_received", sessionWakeId);
    printf("[voice] id=%s wake recognized; wake-word mode restored\n", sessionWakeId);
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
  speechQueue = xQueueCreate(VOICE_EVENT_QUEUE_LENGTH, sizeof(VoiceWorkerMessage));
  if (speechQueue == nullptr) {
    puts("ERROR: voice speech-event queue allocation failed");
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

  voicePrefsReady = voicePrefs.begin(VOICE_PREFS_NAMESPACE, false);
  if (voicePrefsReady) {
    const uint8_t activeSlot = voicePrefs.getUChar("slot", 0);
    if (activeSlot <= 1 && read_config_slot(activeSlot, activeConfig)) {
      printf("[voice] loaded voice settings revision=%u\n", activeConfig.revision);
    } else if (voicePrefs.getBytesLength(activeSlot == 0 ? "voice0" : "voice1") != 0) {
      puts("[voice] saved voice settings invalid; using built-in commands");
      activeConfig = {};
    }
  } else {
    puts("[voice] settings storage unavailable; custom voice sync disabled");
  }
  callbackVoiceRevision.store(activeConfig.revision, std::memory_order_relaxed);
  ESP_SR.onEvent(on_speech_event);
  if (!start_recognizer(activeConfig)) {
    puts("[voice] saved phrase table rejected; falling back to built-ins");
    activeConfig = {};
    callbackVoiceRevision.store(0, std::memory_order_relaxed);
    if (!start_recognizer(activeConfig)) {
      puts("ERROR: ESP_SR initialization failed");
      microphone.end();
      return false;
    }
  }
  if (xTaskCreate(voice_worker, "voice_wake", VOICE_WORKER_STACK_BYTES,
                  nullptr, 1, nullptr) != pdPASS) {
    puts("ERROR: voice worker start failed");
    if (ESP_SR.end()) release_model_reference();
    microphone.end();
    return false;
  }
  puts("[voice] listening for Jarvis");
  return true;
}

bool voice_service_apply_config(const VoiceConfigSnapshot& config) {
  if (!speechQueue || !voice_config_is_valid(config)) return false;
  ulTaskNotifyTake(pdTRUE, 0);
  VoiceConfigUpdate update = {config, xTaskGetCurrentTaskHandle(), false};
  VoiceWorkerMessage message = {};
  message.kind = WorkerMessageKind::ConfigUpdate;
  message.update = &update;
  if (xQueueSend(speechQueue, &message, 0) != pdPASS) return false;
  // The voice worker may defer activation until the current command window ends.
  ulTaskNotifyTake(pdTRUE, portMAX_DELAY);
  return update.success;
}
