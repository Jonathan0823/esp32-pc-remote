#pragma once

#include <stdint.h>

constexpr uint32_t OTA_BUTTON_DEBOUNCE_MS = 50;
constexpr uint32_t OTA_BUTTON_HOLD_MS = 3000;
constexpr uint32_t OTA_WINDOW_MS = 120000;
constexpr uint32_t OTA_TRANSFER_TIMEOUT_MS = 180000;

enum class OtaState { Closed, WindowOpen, Transferring };

class OtaButtonGate {
 public:
  void begin(bool pressed, uint32_t now) {
    armed_ = !pressed;
    candidatePressed_ = pressed;
    stablePressed_ = pressed;
    holdConsumed_ = pressed;
    candidateSince_ = now;
    pressedSince_ = now;
  }

  bool sample(bool pressed, uint32_t now) {
    if (pressed != candidatePressed_) {
      candidatePressed_ = pressed;
      candidateSince_ = now;
    }
    if (candidatePressed_ != stablePressed_ &&
        now - candidateSince_ >= OTA_BUTTON_DEBOUNCE_MS) {
      stablePressed_ = candidatePressed_;
      if (!armed_) {
        if (!stablePressed_) {
          armed_ = true;
          holdConsumed_ = false;
        }
      } else if (stablePressed_) {
        pressedSince_ = now;
        holdConsumed_ = false;
      } else {
        holdConsumed_ = false;
      }
    }
    if (!armed_ || !stablePressed_ || holdConsumed_ ||
        now - pressedSince_ < OTA_BUTTON_HOLD_MS) {
      return false;
    }
    holdConsumed_ = true;
    return true;
  }

 private:
  bool armed_ = false;
  bool candidatePressed_ = false;
  bool stablePressed_ = false;
  bool holdConsumed_ = false;
  uint32_t candidateSince_ = 0;
  uint32_t pressedSince_ = 0;
};

class OtaPolicy {
 public:
  bool open_window(uint32_t now, bool configured, bool wifiConnected) {
    if (state_ != OtaState::Closed || !configured || !wifiConnected) return false;
    state_ = OtaState::WindowOpen;
    windowStartedAt_ = now;
    return true;
  }

  bool expire_window(uint32_t now) {
    if (state_ != OtaState::WindowOpen || now - windowStartedAt_ < OTA_WINDOW_MS) {
      return false;
    }
    state_ = OtaState::Closed;
    return true;
  }

  bool begin_transfer(uint32_t now, bool configured, bool wifiConnected,
                      bool appCommand) {
    if (state_ != OtaState::WindowOpen || !configured || !wifiConnected ||
        !appCommand || now - windowStartedAt_ >= OTA_WINDOW_MS) {
      if (state_ == OtaState::WindowOpen && now - windowStartedAt_ >= OTA_WINDOW_MS) {
        state_ = OtaState::Closed;
      }
      return false;
    }
    state_ = OtaState::Transferring;
    transferStartedAt_ = now;
    return true;
  }

  bool transfer_expired(uint32_t now) const {
    return state_ == OtaState::Transferring &&
           now - transferStartedAt_ >= OTA_TRANSFER_TIMEOUT_MS;
  }

  void finish_transfer() { state_ = OtaState::Closed; }
  void close() { state_ = OtaState::Closed; }
  OtaState state() const { return state_; }

 private:
  OtaState state_ = OtaState::Closed;
  uint32_t windowStartedAt_ = 0;
  uint32_t transferStartedAt_ = 0;
};
