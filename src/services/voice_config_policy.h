#pragma once

#include <stdint.h>
#include <string.h>

constexpr uint16_t VOICE_CONFIG_SCHEMA = 1;
constexpr uint8_t VOICE_CONFIG_MAX_COMMANDS = 12;
constexpr uint16_t VOICE_CONFIG_RESPONSE_MAX = 3072;
constexpr uint8_t VOICE_ACTION_ID_MAX_BYTES = 64;
constexpr uint8_t VOICE_PHRASE_MAX_BYTES = 63;

struct VoiceConfigCommand {
  char phrase[VOICE_PHRASE_MAX_BYTES + 1];
  char actionId[VOICE_ACTION_ID_MAX_BYTES + 1];
};

struct VoiceConfigSnapshot {
  uint16_t schema;
  uint16_t count;
  uint32_t revision;
  VoiceConfigCommand commands[VOICE_CONFIG_MAX_COMMANDS];
};

inline char voice_ascii_lower(char value) {
  return value >= 'A' && value <= 'Z' ? static_cast<char>(value + ('a' - 'A')) : value;
}

inline void voice_phrase_normalize(char* phrase) {
  for (size_t i = 0; phrase && phrase[i] != '\0'; ++i) {
    phrase[i] = voice_ascii_lower(phrase[i]);
  }
}

inline bool voice_phrase_is_valid(const char* phrase) {
  if (!phrase) return false;
  size_t length = 0;
  for (; phrase[length] != '\0' && length <= VOICE_PHRASE_MAX_BYTES; ++length) {
    const char ch = phrase[length];
    if (!((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch == ' ')) return false;
    if (ch == ' ' && (length == 0 || phrase[length - 1] == ' ')) return false;
  }
  return length >= 2 && length <= VOICE_PHRASE_MAX_BYTES && phrase[length - 1] != ' ';
}

inline bool voice_action_id_is_valid(const char* id) {
  if (!id || id[0] < 'a' || id[0] > 'z') return false;
  size_t length = 0;
  for (; id[length] != '\0' && length <= VOICE_ACTION_ID_MAX_BYTES; ++length) {
    const char ch = id[length];
    if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '_')) return false;
  }
  return length >= 1 && length <= VOICE_ACTION_ID_MAX_BYTES;
}

inline bool voice_phrase_reserved(const char* phrase) {
  static const char* const reserved[] = {
      "jarvis", "wake", "play", "pause", "next", "back", "mute", "unmute", "sync"};
  for (const char* value : reserved) {
    size_t i = 0;
    while (phrase[i] && value[i] && voice_ascii_lower(phrase[i]) == value[i]) ++i;
    if (!phrase[i] && !value[i]) return true;
  }
  return false;
}

inline bool voice_phrases_equal(const char* left, const char* right) {
  size_t i = 0;
  while (left[i] && right[i] && voice_ascii_lower(left[i]) == voice_ascii_lower(right[i])) ++i;
  return left[i] == '\0' && right[i] == '\0';
}

inline bool voice_config_is_valid(const VoiceConfigSnapshot& config) {
  if (config.schema != VOICE_CONFIG_SCHEMA || config.revision == 0 ||
      config.count > VOICE_CONFIG_MAX_COMMANDS) return false;
  for (uint16_t i = 0; i < config.count; ++i) {
    const VoiceConfigCommand& command = config.commands[i];
    if (!memchr(command.phrase, '\0', sizeof(command.phrase)) ||
        !memchr(command.actionId, '\0', sizeof(command.actionId)) ||
        !voice_phrase_is_valid(command.phrase) || voice_phrase_reserved(command.phrase) ||
        !voice_action_id_is_valid(command.actionId)) return false;
    for (uint16_t j = 0; j < i; ++j) {
      if (voice_phrases_equal(command.phrase, config.commands[j].phrase) ||
          strcmp(command.actionId, config.commands[j].actionId) == 0) return false;
    }
  }
  return true;
}
