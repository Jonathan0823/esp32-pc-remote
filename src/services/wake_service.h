#ifndef WAKE_SERVICE_H
#define WAKE_SERVICE_H

#include <Arduino.h>

unsigned long wake_timeout_seconds();
void wake_send_magic(const String& mac, const String& bcast, int port);
bool wake_is_pc_reachable(const String& ip, int port);
void wake_start_polling(String chatId, const String& deviceName, const String& ip, int probePort);
unsigned long wake_elapsed_seconds();
String wake_chat_id();

#endif
