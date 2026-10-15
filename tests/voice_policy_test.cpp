#include <assert.h>
#include <stdint.h>

#include "../src/services/voice_policy.h"

int main() {
  assert(voice_wake_allowed(100, true, false, 0));
  assert(!voice_wake_allowed(100, false, false, 0));
  assert(!voice_wake_allowed(5999, true, true, 1000));
  assert(voice_wake_allowed(6000, true, true, 1000));

  const uint32_t beforeWrap = UINT32_MAX - 2000;
  assert(!voice_wake_allowed(beforeWrap + 4999U, true, true, beforeWrap));
  assert(voice_wake_allowed(beforeWrap + 5000U, true, true, beforeWrap));
  assert(!voice_wake_allowed(beforeWrap + 6000U, false, true, beforeWrap));

  assert(voice_led_pulse_active(200, 200));
  assert(voice_led_pulse_active(699, 200));
  assert(!voice_led_pulse_active(700, 200));
  const uint32_t nearWrap = UINT32_MAX - 100;
  assert(voice_led_pulse_active(nearWrap + 499U, nearWrap));
  assert(!voice_led_pulse_active(nearWrap + 500U, nearWrap));
}
