#ifndef LOG_SERVICE_H
#define LOG_SERVICE_H

#include <Arduino.h>

void log_init();
void log_event(const char* level, const char* component, const char* event, const char* msg);
void log_heartbeat(const String& targetName);

#endif
