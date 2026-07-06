#include <WiFi.h>
#include <WiFiUdp.h>
#include <UniversalTelegramBot.h>
#include "config.h"
#include "src/services/wake_service.h"

extern UniversalTelegramBot bot;

static WiFiUDP wakeUdp;

static bool wakePending = false;
static unsigned long wakeStartMs = 0;
static unsigned long lastWakeCheck = 0;
static String wakeChatId = "";
static String wakeDeviceName = "";
static String wakeDeviceIp = "";
static int wakeDeviceProbePort = 0;

static const unsigned long WAKE_TIMEOUT_MS = 90000;
static const unsigned long WAKE_RETRY_MS = 3000;

static void wake_clear() {
  wakePending = false;
  wakeChatId = "";
  wakeDeviceName = "";
  wakeDeviceIp = "";
  wakeDeviceProbePort = 0;
  wakeStartMs = 0;
  lastWakeCheck = 0;
}

void wake_poll() {
  if (!wakePending) return;
  if (WiFi.status() != WL_CONNECTED) return;
  if (millis() - lastWakeCheck < WAKE_RETRY_MS) return;

  if (wake_is_pc_reachable(wakeDeviceIp, wakeDeviceProbePort)) {
    bot.sendMessage(wakeChatId,
                    "🖥 " + wakeDeviceName + " is now online! (took "
                    + String((millis() - wakeStartMs) / 1000) + "s)",
                    "");
    wake_clear();
    return;
  }

  if (millis() - wakeStartMs >= WAKE_TIMEOUT_MS) {
    bot.sendMessage(wakeChatId,
                    "⚠️ " + wakeDeviceName + " did not wake within "
                    + String(WAKE_TIMEOUT_MS / 1000)
                    + "s. Check BIOS WoL settings.",
                    "");
    wake_clear();
    return;
  }

  lastWakeCheck = millis();
}

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

bool wake_is_pc_reachable(const String& ip, int port) {
  WiFiClient probe;
  uint32_t start = millis();
  Serial.printf("[wake] probe %s:%d start\n", ip.c_str(), port);
  bool r = probe.connect(ip.c_str(), port, 750);
  probe.stop();
  Serial.printf("[wake] probe %s:%d %s in %lums\n",
                ip.c_str(),
                port,
                r ? "up" : "down",
                millis() - start);
  return r;
}

void wake_start_polling(String chatId, const String& deviceName, const String& ip, int probePort) {
  wakePending = true;
  wakeStartMs = millis();
  lastWakeCheck = millis();
  wakeChatId = chatId;
  wakeDeviceName = deviceName;
  wakeDeviceIp = ip;
  wakeDeviceProbePort = probePort;
  Serial.printf("[wake] pending target=%s chat=%s\n", deviceName.c_str(), chatId.c_str());
}

unsigned long wake_timeout_seconds() {
  return WAKE_TIMEOUT_MS / 1000;
}
