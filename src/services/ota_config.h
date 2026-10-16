#pragma once

#include <stddef.h>

#ifndef PC_OTA_PASSWORD
#define PC_OTA_PASSWORD ""
#endif

enum class OtaConfigStatus { Disabled, Invalid, Ready };

inline OtaConfigStatus ota_config_status(const char* password) {
  if (!password || !password[0]) return OtaConfigStatus::Disabled;
  size_t length = 0;
  while (length <= 64 && password[length]) ++length;
  if (length != 64) return OtaConfigStatus::Invalid;
  for (size_t i = 0; i < length; ++i) {
    const char c = password[i];
    if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) {
      return OtaConfigStatus::Invalid;
    }
  }
  return OtaConfigStatus::Ready;
}

inline OtaConfigStatus ota_config_status() {
  return ota_config_status(PC_OTA_PASSWORD);
}
