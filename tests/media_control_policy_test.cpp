#include <assert.h>
#include <stdint.h>
#include <string.h>

#include "../src/services/media_control_policy.h"
#include "../src/services/media_control_config.h"

int main() {
  assert(media_control_config_status() == MediaConfigStatus::Disabled);
  const char* expected[] = {"play", "pause", "next", "previous", "mute", "unmute"};
  const char* phrases[] = {"play", "pause", "next",
                           "back", "mute", "unmute"};
  for (int i = 0; i < 6; ++i) {
    MediaCommand command;
    assert(media_command_from_id(i + MEDIA_COMMAND_FIRST_ID, command));
    assert(strcmp(media_command_name(command), expected[i]) == 0);
    assert(strcmp(media_command_phrase(command), phrases[i]) == 0);
  }
  MediaCommand command;
  assert(!media_command_from_id(MEDIA_COMMAND_FIRST_ID - 1, command));
  assert(!media_command_from_id(MEDIA_COMMAND_FIRST_ID + 6, command));

  assert(!media_time_is_synchronized(MEDIA_MIN_VALID_UNIX_MS - 1));
  assert(media_time_is_synchronized(MEDIA_MIN_VALID_UNIX_MS));
  assert(media_request_is_fresh(1000, 5999));
  assert(!media_request_is_fresh(1000, 6000));
  assert(media_request_is_fresh(UINT32_MAX - 1000, 3998));

  assert(media_wall_clock_valid(10000, 15000, 10000));
  assert(media_wall_clock_valid(10000, 15000, 14999));
  assert(!media_wall_clock_valid(10000, 15000, 15000));
  assert(media_wall_clock_valid(11000, 16000, 10000));
  assert(!media_wall_clock_valid(11001, 16001, 10000));
  assert(!media_wall_clock_valid(10000, 14999, 10000));
  assert(!media_wall_clock_valid(UINT64_MAX - 10, UINT64_MAX, UINT64_MAX - 10));

  assert(media_classify_response(202, true, "accepted") == MediaResult::Accepted);
  assert(media_classify_response(200, true, "executed") == MediaResult::Executed);
  assert(media_classify_response(202, true, "executed") == MediaResult::Unknown);
  assert(media_classify_response(200, true, "accepted") == MediaResult::Unknown);
  assert(media_classify_response(200, false, "executed") == MediaResult::Unknown);
  assert(media_classify_response(401, true, "rejected") == MediaResult::Rejected);
  assert(media_classify_response(503, true, "error") == MediaResult::Unknown);

  const char* token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
  assert(media_config_status("192.168.1.8", 8443, "ca", token) ==
         MediaConfigStatus::Ready);
  assert(media_config_status("", 8443, "", "") == MediaConfigStatus::Disabled);
  assert(media_config_status("https://pc.local", 8443, "ca", token) ==
         MediaConfigStatus::Invalid);
  assert(media_config_status("pc.local", 0, "ca", token) == MediaConfigStatus::Invalid);
  assert(media_config_status("pc.local", 8443, "", token) == MediaConfigStatus::Invalid);
  assert(media_config_status("pc.local", 8443, "ca", "short") ==
         MediaConfigStatus::Invalid);
  assert(media_config_status("pc.local", 8443, "ca",
                             "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg") ==
         MediaConfigStatus::Invalid);
  return 0;
}
