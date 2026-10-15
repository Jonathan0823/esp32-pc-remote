#include <assert.h>
#include <stdint.h>

#include "../src/services/voice_policy.h"

int main() {
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

  assert(voice_led_pulse_active(200, 200));
  assert(voice_led_pulse_active(699, 200));
  assert(!voice_led_pulse_active(700, 200));
  const uint32_t nearWrap = UINT32_MAX - 100;
  assert(voice_led_pulse_active(nearWrap + 499U, nearWrap));
  assert(!voice_led_pulse_active(nearWrap + 500U, nearWrap));
}
