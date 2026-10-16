#include <assert.h>

#include "../src/services/ota_config.h"

int main() {
  assert(ota_config_status() == OtaConfigStatus::Ready);
  assert(ota_config_status("") == OtaConfigStatus::Disabled);
  assert(ota_config_status(nullptr) == OtaConfigStatus::Disabled);
  assert(ota_config_status("too-short") == OtaConfigStatus::Invalid);
  assert(ota_config_status("0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF") ==
         OtaConfigStatus::Invalid);
  return 0;
}
