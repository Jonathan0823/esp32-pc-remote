#pragma once

#include <stdint.h>

constexpr uint32_t VOICE_WAKE_COOLDOWN_MS = 5000;
constexpr uint32_t VOICE_LED_PULSE_MS = 500;

inline bool voice_wake_allowed(uint32_t now, bool wifiConnected,
                               bool hasSuccessfulWake, uint32_t lastSuccessfulWake) {
  return wifiConnected &&
         (!hasSuccessfulWake || now - lastSuccessfulWake >= VOICE_WAKE_COOLDOWN_MS);
}

inline bool voice_led_pulse_active(uint32_t now, uint32_t pulseStarted) {
  return now - pulseStarted < VOICE_LED_PULSE_MS;
}
