#include <assert.h>
#include <stdint.h>
#include <string.h>

#include "../src/services/wol_packet.h"

int main() {
  uint8_t packet[102];
  assert(wol_build_magic_packet("01:23:45:67:89:ab", packet));
  for (int i = 0; i < 6; ++i) assert(packet[i] == 0xff);
  for (int repeat = 0; repeat < 16; ++repeat) {
    assert(memcmp(packet + 6 + repeat * 6, "\x01\x23\x45\x67\x89\xab", 6) == 0);
  }
  assert(wol_build_magic_packet("01:23:45:67:89:AB", packet));
  assert(!wol_build_magic_packet(nullptr, packet));
  assert(!wol_build_magic_packet("01:23:45:67:89", packet));
  assert(!wol_build_magic_packet("01:23:45:67:89:ag", packet));
  assert(!wol_build_magic_packet("0123:45:67:89:ab", packet));
  assert(!wol_build_magic_packet("01-23-45-67-89-ab", packet));
}
