#pragma once

#include <stddef.h>
#include <stdint.h>
#include <string.h>

inline int wol_hex_nibble(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

inline bool wol_build_magic_packet(const char* mac, uint8_t packet[102]) {
  if (mac == nullptr || packet == nullptr || strlen(mac) != 17) return false;

  uint8_t address[6];
  for (size_t i = 0; i < 6; ++i) {
    const size_t offset = i * 3;
    const int high = wol_hex_nibble(mac[offset]);
    const int low = wol_hex_nibble(mac[offset + 1]);
    if (high < 0 || low < 0 || (i < 5 && mac[offset + 2] != ':')) return false;
    address[i] = static_cast<uint8_t>((high << 4) | low);
  }

  memset(packet, 0xFF, 6);
  for (size_t i = 0; i < 16; ++i) {
    memcpy(packet + 6 + i * sizeof(address), address, sizeof(address));
  }
  return true;
}
