#include "config.h"
#include "device_service.h"

#ifndef PC_TCP_PORT
#define PC_TCP_PORT 47989
#endif

static const Device device = {
  PC_NAME,
  PC_MAC,
  PC_IP,
  WOL_BCAST,
  WOL_PORT,
  PC_TCP_PORT
};

void device_init() {
  // ponytail: one configured PC; no registry needed yet.
}

const Device& device_get_active() {
  return device;
}

String device_active_name() {
  return device.name;
}
