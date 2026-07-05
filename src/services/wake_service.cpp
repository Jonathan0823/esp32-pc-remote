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

void wake_send_magic(const String& mac, const String& bcast, int port) {
  byte macBytes[6];
  sscanf(mac.c_str(), "%hhx:%hhx:%hhx:%hhx:%hhx:%hhx",
         &macBytes[0], &macBytes[1], &macBytes[2],
         &macBytes[3], &macBytes[4], &macBytes[5]);
  byte packet[102];
  memset(packet, 0xFF, 6);
  for (int i = 0; i < 16; i++) memcpy(&packet[6 + i * 6], macBytes, 6);
  IPAddress bcastAddr;
  bcastAddr.fromString(bcast);
  wakeUdp.beginPacket(bcastAddr, port);
  wakeUdp.write(packet, 102);
  wakeUdp.endPacket();
  Serial.println("WoL sent to " + mac + " via "
                 + bcast + ":" + String(port));
}

// ponytail: TCP probe instead of ICMP ping — swap if PC firewall blocks TCP
bool wake_is_pc_reachable(const String& ip, int port) {
  WiFiClient probe;
  bool r = probe.connect(ip.c_str(), port, 750);
  probe.stop();
  return r;
}

void wake_start_polling(String chatId) {
  wakePending = true;
  wakeStartMs = millis();
  lastWakeCheck = millis();
  wakeChatId = chatId;
}

bool wake_is_pending() {
  return wakePending;
}

int wake_tick(const String& ip, int probePort) {
  if (!wakePending || millis() - lastWakeCheck < WAKE_RETRY_MS) return 0;
  lastWakeCheck = millis();
  if (wake_is_pc_reachable(ip, probePort)) {
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
