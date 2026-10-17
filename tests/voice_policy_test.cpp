#include <assert.h>
#include <stdint.h>

#include "../src/services/voice_policy.h"

int main() {
  assert(voice_command_from_id(1).kind == VoiceCommandKind::Wake);
  assert(voice_command_from_id(VOICE_SYNC_COMMAND_ID).kind == VoiceCommandKind::Sync);
  for (int index = 0; index < VOICE_CONFIG_MAX_COMMANDS; ++index) {
    const VoiceCommand command = voice_command_from_id(VOICE_CUSTOM_COMMAND_FIRST_ID + index);
    assert(command.kind == VoiceCommandKind::Custom);
    assert(command.customIndex == index);
  }
  assert(voice_command_from_id(VOICE_CUSTOM_COMMAND_FIRST_ID + VOICE_CONFIG_MAX_COMMANDS).kind == VoiceCommandKind::Invalid);
  for (int id = MEDIA_COMMAND_FIRST_ID; id < MEDIA_COMMAND_FIRST_ID + 6; ++id) {
    assert(voice_command_from_id(id).kind == VoiceCommandKind::Media);
    assert(static_cast<int>(voice_command_from_id(id).media) == id - MEDIA_COMMAND_FIRST_ID);
  }
  assert(voice_command_from_id(0).kind == VoiceCommandKind::Invalid);
  assert(voice_command_from_id(VOICE_SYNC_COMMAND_ID).kind == VoiceCommandKind::Sync);
  assert(voice_command_from_id(VOICE_CUSTOM_COMMAND_FIRST_ID + VOICE_CONFIG_MAX_COMMANDS).kind == VoiceCommandKind::Invalid);

  VoiceSession session = {};
  assert(!voice_session_accept(session, 0));
  assert(!voice_session_expire(session, 0));

  assert(voice_session_begin(session, 100));
  assert(session.listening);
  assert(session.startedAt == 100);
  assert(!voice_session_begin(session, 200));
  assert(voice_session_accept(session, 5099));
  assert(!session.listening);

  assert(voice_session_begin(session, 6000));
  assert(voice_session_accept(session, 10999));
  assert(!session.listening);
  assert(!voice_session_accept(session, 11000));

  assert(voice_session_begin(session, 12000));
  assert(!voice_session_expire(session, 17759));
  assert(voice_session_expire(session, 17760));
  assert(!session.listening);
  assert(!voice_session_accept(session, 17000));

  assert(voice_session_begin(session, 18000));
  assert(voice_session_accept(session, 23759));

  const uint32_t beforeWrap = UINT32_MAX - 2000;
  assert(voice_session_begin(session, beforeWrap));
  assert(!voice_session_accept(session, beforeWrap + VOICE_LISTEN_WINDOW_MS));
  assert(voice_session_begin(session, beforeWrap + VOICE_LISTEN_WINDOW_MS));
  assert(!voice_session_expire(session, beforeWrap + (2 * VOICE_LISTEN_WINDOW_MS) - 1));
  assert(voice_session_expire(session, beforeWrap + (2 * VOICE_LISTEN_WINDOW_MS)));
}
