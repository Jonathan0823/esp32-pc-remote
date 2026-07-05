#include <Preferences.h>
#include "config.h"
#include "device_service.h"

#ifndef PC_TCP_PORT
#define PC_TCP_PORT 47989
#endif

static const Device devices[] = {
  {PC_NAME, PC_MAC, PC_IP, WOL_BCAST, WOL_PORT, PC_TCP_PORT}
};
static const int DEVICE_COUNT = 1;
static int activeIndex = 0;
static Preferences prefs;

void device_init() {
  prefs.begin("devices", false);
  activeIndex = prefs.getInt("active", 0);
  if (activeIndex < 0 || activeIndex >= DEVICE_COUNT) {
    activeIndex = 0;
  }
}

int device_count() {
  return DEVICE_COUNT;
}

const Device& device_get(int index) {
  return devices[index];
}

const Device& device_get_active() {
  return devices[activeIndex];
}

int device_active_index() {
  return activeIndex;
}

String device_active_name() {
  return devices[activeIndex].name;
}

int device_find(const String& name) {
  for (int i = 0; i < DEVICE_COUNT; i++) {
    if (devices[i].name == name) return i;
  }
  return -1;
}

// ponytail: single global active index; swap to per-chat state if multi-user needed
bool device_set_active(int index) {
  if (index < 0 || index >= DEVICE_COUNT) return false;
  activeIndex = index;
  prefs.putInt("active", activeIndex);
  return true;
}
