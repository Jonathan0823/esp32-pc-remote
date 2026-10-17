#pragma once

#include <stdint.h>
#include "media_control_policy.h"
#include "voice_config_policy.h"

constexpr uint32_t VOICE_LISTEN_WINDOW_MS = 5760;

constexpr int VOICE_WAKE_COMMAND_ID = 1;
constexpr int VOICE_SYNC_COMMAND_ID = 8;
constexpr int VOICE_CUSTOM_COMMAND_FIRST_ID = 100;

enum class VoiceCommandKind : uint8_t { Invalid, Wake, Media, Sync, Custom };

struct VoiceCommand {
  VoiceCommandKind kind;
  MediaCommand media;
  uint8_t customIndex;
};

inline VoiceCommand voice_command_from_id(int commandId) {
  if (commandId == VOICE_WAKE_COMMAND_ID) {
    return {VoiceCommandKind::Wake, MediaCommand::Play, 0};
  }
  if (commandId == VOICE_SYNC_COMMAND_ID) {
    return {VoiceCommandKind::Sync, MediaCommand::Play, 0};
  }
  if (commandId >= VOICE_CUSTOM_COMMAND_FIRST_ID &&
      commandId < VOICE_CUSTOM_COMMAND_FIRST_ID + VOICE_CONFIG_MAX_COMMANDS) {
    return {VoiceCommandKind::Custom, MediaCommand::Play,
            static_cast<uint8_t>(commandId - VOICE_CUSTOM_COMMAND_FIRST_ID)};
  }
  MediaCommand media;
  if (media_command_from_id(commandId, media)) {
    return {VoiceCommandKind::Media, media, 0};
  }
  return {VoiceCommandKind::Invalid, MediaCommand::Play, 0};
}

struct VoiceSession {
  uint32_t startedAt;
  bool listening;
};

inline bool voice_session_begin(VoiceSession& session, uint32_t now) {
  if (session.listening) return false;
  session.startedAt = now;
  session.listening = true;
  return true;
}

inline bool voice_session_accept(VoiceSession& session, uint32_t now) {
  if (!session.listening) return false;
  const bool accepted = now - session.startedAt < VOICE_LISTEN_WINDOW_MS;
  session.listening = false;
  return accepted;
}

inline bool voice_session_expire(VoiceSession& session, uint32_t now) {
  if (!session.listening || now - session.startedAt < VOICE_LISTEN_WINDOW_MS) return false;
  session.listening = false;
  return true;
}
