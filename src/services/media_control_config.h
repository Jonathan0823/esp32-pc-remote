#pragma once

#include <stddef.h>
#include <stdint.h>

#ifndef PC_MEDIA_HOST
#define PC_MEDIA_HOST ""
#endif
#ifndef PC_MEDIA_PORT
#define PC_MEDIA_PORT 8443
#endif
#ifndef PC_MEDIA_CA_CERT
#define PC_MEDIA_CA_CERT ""
#endif
#ifndef PC_MEDIA_API_TOKEN
#define PC_MEDIA_API_TOKEN ""
#endif

enum class MediaConfigStatus { Disabled, Invalid, Ready };

inline bool media_config_host_valid(const char* host) {
  if (!host || !host[0]) return false;
  size_t length = 0;
  bool labelStart = true;
  char previous = '\0';
  for (; host[length]; ++length) {
    const char c = host[length];
    if (length >= 253) return false;
    const bool alnum = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                       (c >= '0' && c <= '9');
    if (alnum) {
      labelStart = false;
    } else if (c == '-') {
      if (labelStart) return false;
    } else if (c == '.') {
      if (labelStart || previous == '-') return false;
      labelStart = true;
    } else {
      return false;
    }
    previous = c;
  }
  return length > 0 && !labelStart && previous != '-';
}

inline bool media_config_token_valid(const char* token) {
  if (!token) return false;
  size_t length = 0;
  while (token[length] && length <= 64) ++length;
  if (length != 64) return false;
  for (size_t i = 0; i < length; ++i) {
    const char c = token[i];
    if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) return false;
  }
  return true;
}

inline MediaConfigStatus media_config_status(const char* host, uint32_t port,
                                             const char* caCert, const char* token) {
  if (!host || !host[0]) return MediaConfigStatus::Disabled;
  if (!media_config_host_valid(host) || port == 0 || port > 65535 ||
      !caCert || !caCert[0] || !media_config_token_valid(token)) {
    return MediaConfigStatus::Invalid;
  }
  return MediaConfigStatus::Ready;
}

inline MediaConfigStatus media_control_config_status() {
  return media_config_status(PC_MEDIA_HOST, PC_MEDIA_PORT,
                             PC_MEDIA_CA_CERT, PC_MEDIA_API_TOKEN);
}
