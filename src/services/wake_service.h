#ifndef WAKE_SERVICE_H
#define WAKE_SERVICE_H

#include <Arduino.h>

unsigned long wake_timeout_seconds();
void wake_send_magic();
bool wake_is_pc_reachable();
void wake_start_polling(String chatId);
// Returns: 0 = no event, 1 = PC online, -1 = timeout
int wake_tick();
unsigned long wake_elapsed_seconds();
String wake_chat_id();

#endif
