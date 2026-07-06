#ifndef DEVICE_SERVICE_H
#define DEVICE_SERVICE_H

#include <Arduino.h>

struct Device {
  String name;
  String mac;
  String ip;
  String bcast;
  int wolPort;
  int probePort;
};

void device_init();
const Device& device_get_active();
String device_active_name();

#endif
