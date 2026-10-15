#pragma once

#include <stdint.h>

constexpr uint32_t VOICE_LISTEN_WINDOW_MS = 5760;

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
