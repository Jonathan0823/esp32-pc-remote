#pragma once

#include <stdint.h>
#include <string.h>

constexpr int MEDIA_COMMAND_FIRST_ID = 2;
constexpr uint32_t MEDIA_COMMAND_LIFETIME_MS = 5000;
constexpr uint64_t MEDIA_FUTURE_SKEW_MS = 1000;
constexpr uint64_t MEDIA_MIN_VALID_UNIX_MS = 1735689600000ULL;
constexpr uint32_t MEDIA_REQUEST_BODY_MAX = 512;
constexpr uint32_t MEDIA_RESPONSE_BODY_MAX = 512;
constexpr uint32_t MEDIA_DUPLICATE_CACHE_MAX = 128;

enum class MediaCommand : uint8_t { Play, Pause, Next, Previous, Mute, Unmute };
enum class MediaResult : uint8_t { Queued, Accepted, Executed, Rejected, Unknown };

inline bool media_command_from_id(int id, MediaCommand& command) {
  if (id < MEDIA_COMMAND_FIRST_ID || id >= MEDIA_COMMAND_FIRST_ID + 6) return false;
  command = static_cast<MediaCommand>(id - MEDIA_COMMAND_FIRST_ID);
  return true;
}

inline const char* media_command_name(MediaCommand command) {
  static const char* const names[] = {"play", "pause", "next", "previous", "mute", "unmute"};
  const unsigned index = static_cast<unsigned>(command);
  return index < sizeof(names) / sizeof(names[0]) ? names[index] : "";
}

inline const char* media_command_phrase(MediaCommand command) {
  static const char* const phrases[] = {
      "play", "pause", "next",
      "back", "mute", "unmute"};
  const unsigned index = static_cast<unsigned>(command);
  return index < sizeof(phrases) / sizeof(phrases[0]) ? phrases[index] : "";
}

inline bool media_time_is_synchronized(uint64_t nowMs) {
  return nowMs >= MEDIA_MIN_VALID_UNIX_MS;
}

inline bool media_request_is_fresh(uint32_t recognizedAtMs, uint32_t nowMs) {
  return nowMs - recognizedAtMs < MEDIA_COMMAND_LIFETIME_MS;
}

inline bool media_wall_clock_valid(uint64_t issuedAtMs, uint64_t expiresAtMs,
                                   uint64_t nowMs) {
  const uint64_t max = UINT64_MAX;
  if (issuedAtMs > max - MEDIA_COMMAND_LIFETIME_MS ||
      expiresAtMs != issuedAtMs + MEDIA_COMMAND_LIFETIME_MS ||
      expiresAtMs <= nowMs) return false;
  return issuedAtMs <= nowMs || issuedAtMs - nowMs <= MEDIA_FUTURE_SKEW_MS;
}

inline MediaResult media_classify_response(int statusCode, bool requestIdMatches,
                                           const char* status) {
  if (statusCode >= 400 && statusCode < 500) return MediaResult::Rejected;
  if (!requestIdMatches || status == nullptr) return MediaResult::Unknown;
  if (statusCode == 202 && strcmp(status, "accepted") == 0) return MediaResult::Accepted;
  if (statusCode == 200 && strcmp(status, "executed") == 0) return MediaResult::Executed;
  return MediaResult::Unknown;
}
