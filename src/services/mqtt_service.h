#ifndef MQTT_SERVICE_H
#define MQTT_SERVICE_H

#include <Arduino.h>

// Payload shapes match dashboard/src/mqtt/types.ts

void mqtt_setup();
void mqtt_loop();
bool mqtt_enabled();
bool mqtt_connected();

#endif
