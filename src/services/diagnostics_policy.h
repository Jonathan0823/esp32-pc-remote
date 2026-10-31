#pragma once

#include <stdint.h>

constexpr uint8_t DIAGNOSTICS_RING_CAPACITY = 32;
constexpr uint8_t DIAGNOSTICS_BATCH_CAPACITY = 8;
constexpr uint32_t DIAGNOSTICS_MAX_BACKOFF_MS = 30000;

inline uint32_t diagnostics_retry_delay_ms(uint8_t failures) {
  uint32_t delay = 1000;
  while (failures > 0 && delay < DIAGNOSTICS_MAX_BACKOFF_MS) {
    --failures;
    delay = delay > DIAGNOSTICS_MAX_BACKOFF_MS / 2
                ? DIAGNOSTICS_MAX_BACKOFF_MS
                : delay * 2;
  }
  return delay;
}

enum class DiagnosticKind : uint8_t { ClockState, PreflightRejected, HttpResponse, TlsTransport };
enum class DiagnosticCategory : uint8_t {
  Ready, NotReady, WifiUnavailable, ClockUnavailable, Stale, QueueFull,
  Http2xx, Http4xx, Http5xx, HttpOther, Dns, Connect, Certificate, Handshake, Unknown
};
struct DiagnosticRecord {
  uint32_t uptimeMs;
  uint64_t unixTimeMs;
  bool hasUnixTime;
  DiagnosticKind kind;
  DiagnosticCategory category;
};

class DiagnosticRing {
 public:
  void push(const DiagnosticRecord& record) {
    if (size_ == DIAGNOSTICS_RING_CAPACITY) {
      head_ = static_cast<uint8_t>((head_ + 1) % DIAGNOSTICS_RING_CAPACITY);
      --size_;
      ++dropped_;
    }
    records_[(head_ + size_) % DIAGNOSTICS_RING_CAPACITY] = record;
    ++size_;
  }
  uint8_t copy(DiagnosticRecord* output, uint8_t maximum) const {
    const uint8_t count = size_ < maximum ? size_ : maximum;
    for (uint8_t i = 0; i < count; ++i) output[i] = records_[(head_ + i) % DIAGNOSTICS_RING_CAPACITY];
    return count;
  }
  void discard(uint8_t count) {
    if (count > size_) count = size_;
    head_ = static_cast<uint8_t>((head_ + count) % DIAGNOSTICS_RING_CAPACITY);
    size_ = static_cast<uint8_t>(size_ - count);
  }
  uint8_t size() const { return size_; }
  uint32_t dropped() const { return dropped_; }
 private:
  DiagnosticRecord records_[DIAGNOSTICS_RING_CAPACITY] = {};
  uint8_t head_ = 0;
  uint8_t size_ = 0;
  uint32_t dropped_ = 0;
};
