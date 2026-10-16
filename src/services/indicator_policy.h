#pragma once

#include <stdint.h>
#include <string.h>
#include "media_control_policy.h"

constexpr uint32_t INDICATOR_PULSE_MS = 500;

struct IndicatorColor {
  uint8_t red;
  uint8_t green;
  uint8_t blue;
};

inline bool indicator_pulse_active(uint32_t now, uint32_t pulseStarted) {
  return now - pulseStarted < INDICATOR_PULSE_MS;
}

inline IndicatorColor indicator_listening_color(uint8_t brightness) {
  return {0, 0, brightness};
}

inline IndicatorColor indicator_wake_color(const char* source, bool sent,
                                           uint8_t brightness) {
  if (!sent) return {brightness, 0, 0};
  if (source && strcmp(source, "mqtt") == 0) return {0, brightness, brightness};
  if (source && strcmp(source, "telegram") == 0) return {brightness, 0, brightness};
  return {0, brightness, 0};
}

inline IndicatorColor indicator_media_color(MediaResult result, uint8_t brightness) {
  switch (result) {
    case MediaResult::Queued:
    case MediaResult::Accepted: {
      const uint8_t channel = brightness / 2;
      return {channel, channel, 0};
    }
    case MediaResult::Executed: {
      const uint8_t channel = brightness / 3;
      return {channel, channel, channel};
    }
    case MediaResult::Rejected:
    case MediaResult::Unknown:
      return {brightness, 0, 0};
  }
  return {brightness, 0, 0};
}
