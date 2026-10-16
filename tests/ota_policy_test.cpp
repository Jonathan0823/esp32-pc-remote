#include <assert.h>
#include <stdint.h>

#include "../src/services/ota_policy.h"
#include "../src/services/ota_config.h"

int main() {
  assert(ota_config_status() == OtaConfigStatus::Disabled);
  OtaButtonGate button;
  button.begin(false, 0);
  assert(!button.sample(true, 10));
  assert(!button.sample(false, 30));  // short bounce is ignored
  assert(!button.sample(false, 80));
  assert(!button.sample(true, 100));
  assert(!button.sample(true, 149));
  assert(!button.sample(true, 150));
  assert(!button.sample(true, 3149));
  assert(button.sample(true, 3150));  // 50 ms debounce + exact 3 s hold
  assert(!button.sample(true, 10000));  // one activation per hold
  assert(!button.sample(false, 10010));
  assert(!button.sample(false, 10060));
  assert(!button.sample(true, 10100));
  assert(!button.sample(true, 10150));
  assert(button.sample(true, 13150));

  OtaButtonGate shortPress;
  shortPress.begin(false, 0);
  assert(!shortPress.sample(true, 1));
  assert(!shortPress.sample(true, 51));
  assert(!shortPress.sample(false, 3049));  // released before the hold threshold
  assert(!shortPress.sample(false, 3099));

  button.begin(true, 0);  // held during startup must be released before arming
  assert(!button.sample(true, 5000));
  assert(!button.sample(false, 6000));
  assert(!button.sample(false, 6050));
  assert(!button.sample(false, 9000));
  assert(!button.sample(true, 9001));
  assert(!button.sample(true, 9051));
  assert(button.sample(true, 12051));

  const uint32_t wrapStart = UINT32_MAX - 30;
  button.begin(false, wrapStart);
  assert(!button.sample(true, wrapStart + 1));
  assert(!button.sample(true, wrapStart + 50));
  const uint32_t pressedAt = wrapStart + 51;
  assert(!button.sample(true, pressedAt));
  assert(!button.sample(true, pressedAt + OTA_BUTTON_HOLD_MS - 1));
  assert(button.sample(true, pressedAt + OTA_BUTTON_HOLD_MS));

  OtaPolicy policy;
  assert(!policy.open_window(0, false, true));
  assert(!policy.open_window(0, true, false));
  assert(policy.open_window(100, true, true));
  assert(policy.state() == OtaState::WindowOpen);
  assert(!policy.open_window(101, true, true));
  assert(!policy.begin_transfer(200, false, true, true));
  assert(!policy.begin_transfer(200, true, false, true));
  assert(!policy.begin_transfer(200, true, true, false));  // reject filesystem image
  assert(policy.state() == OtaState::WindowOpen);

  const uint32_t transferStart = 120099;
  assert(policy.begin_transfer(transferStart, true, true, true));  // 1 ms before expiry
  assert(policy.state() == OtaState::Transferring);
  assert(!policy.expire_window(120100));  // admission expiry doesn't abort active transfer
  assert(!policy.transfer_expired(transferStart + OTA_TRANSFER_TIMEOUT_MS - 1));
  assert(policy.transfer_expired(transferStart + OTA_TRANSFER_TIMEOUT_MS));
  policy.finish_transfer();
  assert(policy.state() == OtaState::Closed);

  assert(policy.open_window(UINT32_MAX - 1000, true, true));
  assert(!policy.expire_window(UINT32_MAX - 1000 + OTA_WINDOW_MS - 1));
  assert(policy.expire_window(UINT32_MAX - 1000 + OTA_WINDOW_MS));
  assert(policy.state() == OtaState::Closed);

  // A held button cannot create a deferred window after readiness/Wi-Fi returns.
  button.begin(false, 0);
  assert(!button.sample(true, 0));
  assert(!button.sample(true, OTA_BUTTON_DEBOUNCE_MS));
  const uint32_t activationAt = OTA_BUTTON_DEBOUNCE_MS + OTA_BUTTON_HOLD_MS;
  assert(button.sample(true, activationAt));
  assert(!policy.open_window(activationAt, true, false));
  assert(!button.sample(true, activationAt + 10000));
  assert(policy.state() == OtaState::Closed);
  return 0;
}
