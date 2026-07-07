#ifndef WAKE_SERVICE_H
#define WAKE_SERVICE_H

#include <Arduino.h>

void wake_send_magic(const String& mac, const String& bcast, int port);
bool wake_is_pc_reachable(const String& ip, int port);
void wake_start_polling(String chatId, const String& deviceName, const String& ip, int probePort);
bool wake_is_pending();
unsigned long wake_pending_elapsed_seconds();
String wake_last_result();
unsigned long wake_last_result_age_seconds();
unsigned long wake_last_online_age_seconds();
void wake_mark_online_seen();
void wake_poll();

#endif
