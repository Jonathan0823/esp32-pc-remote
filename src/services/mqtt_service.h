#ifndef MQTT_SERVICE_H
#define MQTT_SERVICE_H

#include <Arduino.h>

void mqtt_setup();
void mqtt_loop();
bool mqtt_enabled();
bool mqtt_connected();

#endif
