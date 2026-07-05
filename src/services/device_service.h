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
int device_count();
const Device& device_get(int index);
const Device& device_get_active();
int device_active_index();
String device_active_name();
int device_find(const String& name);
bool device_set_active(int index);

#endif
