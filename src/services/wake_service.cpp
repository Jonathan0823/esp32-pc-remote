#include <WiFi.h>
#include <WiFiUdp.h>
#include "config.h"
#include "src/services/wake_service.h"

WiFiUDP wakeUdp;

bool wakePending = false;
unsigned long wakeStartMs = 0;
unsigned long lastWakeCheck = 0;
String wakeChatId = "";
const unsigned long WAKE_TIMEOUT_MS = 90000;
const unsigned long WAKE_RETRY_MS = 3000;

void wake_send_magic() {
  byte mac[6];
  sscanf(PC_MAC, "%hhx:%hhx:%hhx:%hhx:%hhx:%hhx",
         &mac[0], &mac[1], &mac[2], &mac[3], &mac[4], &mac[5]);
  byte packet[102];
  memset(packet, 0xFF, 6);
  for (int i = 0; i < 16; i++) memcpy(&packet[6 + i * 6], mac, 6);
  IPAddress bcast;
  bcast.fromString(WOL_BCAST);
  wakeUdp.beginPacket(bcast, WOL_PORT);
  wakeUdp.write(packet, 102);
  wakeUdp.endPacket();
  Serial.println("WoL sent to " + String(PC_MAC) + " via "
                 + String(WOL_BCAST) + ":" + String(WOL_PORT));
}

bool wake_is_pc_reachable() {
  // ponytail: TCP probe to Artemis port; swap to ICMP ping if reliability matters
  WiFiClient probe;
  bool r = probe.connect(PC_IP, 47989, 2000);
  probe.stop();
  return r;
}

void wake_start_polling(String chatId) {
  wakePending = true;
  wakeStartMs = millis();
  lastWakeCheck = millis();
  wakeChatId = chatId;
}

int wake_tick() {
  if (!wakePending || millis() - lastWakeCheck < WAKE_RETRY_MS) return 0;
  lastWakeCheck = millis();
  if (wake_is_pc_reachable()) {
    wakePending = false;
    return 1;
  } else if (millis() - wakeStartMs >= WAKE_TIMEOUT_MS) {
    wakePending = false;
    return -1;
  }
  return 0;
}

unsigned long wake_elapsed_seconds() {
  return (millis() - wakeStartMs) / 1000;
}

String wake_chat_id() {
  return wakeChatId;
}

unsigned long wake_timeout_seconds() {
  return WAKE_TIMEOUT_MS / 1000;
}
