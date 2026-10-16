#pragma once

#include <stdint.h>
#include <WiFiClientSecure.h>
#include "src/services/media_control_policy.h"

enum class MediaSubmitStatus : uint8_t {
  Queued,
  Disabled,
  WifiUnavailable,
  ClockNotReady,
  Stale,
  QueueFull,
  RandomUnavailable
};

void media_control_time_setup();
bool media_control_time_ready();
bool media_control_prepare_tls(WiFiClientSecure& client);
void media_control_service_start();
MediaSubmitStatus media_control_submit(MediaCommand command,
                                       uint32_t recognizedAtMs,
                                       uint64_t issuedAtUnixMs);
