#include <assert.h>
#include <stdint.h>

#include "../src/services/indicator_policy.h"

static bool same(IndicatorColor a, IndicatorColor b) {
  return a.red == b.red && a.green == b.green && a.blue == b.blue;
}

int main() {
  const uint8_t brightness = 42;
  const IndicatorColor red = {brightness, 0, 0};
  const IndicatorColor blue = {0, 0, brightness};
  const IndicatorColor green = {0, brightness, 0};
  const IndicatorColor cyan = {0, brightness, brightness};
  const IndicatorColor purple = {brightness, 0, brightness};

  assert(same(indicator_listening_color(brightness), blue));
  assert(same(indicator_wake_color("voice", true, brightness), green));
  assert(same(indicator_wake_color("mqtt", true, brightness), cyan));
  assert(same(indicator_wake_color("telegram", true, brightness), purple));
  assert(same(indicator_wake_color("unknown", true, brightness), green));
  assert(same(indicator_wake_color("mqtt", false, brightness), red));
  assert(same(indicator_media_color(MediaResult::Queued, brightness),
               {brightness / 2, brightness / 2, 0}));
  assert(same(indicator_media_color(MediaResult::Accepted, brightness),
               {brightness / 2, brightness / 2, 0}));
  assert(same(indicator_media_color(MediaResult::Executed, brightness),
               {brightness / 3, brightness / 3, brightness / 3}));
  assert(same(indicator_media_color(MediaResult::Unknown, brightness), red));
  assert(same(indicator_media_color(MediaResult::Rejected, brightness), red));

  assert(indicator_pulse_active(200, 200));
  assert(indicator_pulse_active(699, 200));
  assert(!indicator_pulse_active(700, 200));
  const uint32_t nearWrap = UINT32_MAX - 100;
  assert(indicator_pulse_active(nearWrap + 499U, nearWrap));
  assert(!indicator_pulse_active(nearWrap + 500U, nearWrap));
}
