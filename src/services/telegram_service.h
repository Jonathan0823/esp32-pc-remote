#ifndef TELEGRAM_SERVICE_H
#define TELEGRAM_SERVICE_H

#include <Arduino.h>

void telegram_setup();
void telegram_poll();
void telegram_send_message(const String& chatId, const String& message, const char* parseMode = "");

#endif
