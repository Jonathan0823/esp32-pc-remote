#include <WiFi.h>
#include <WiFiUdp.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include "config.h"
#include "src/services/wake_service.h"

WiFiUDP wakeUdp;
WiFiClientSecure wakeNotifyClient;
UniversalTelegramBot wakeNotifyBot(BOT_TOKEN, wakeNotifyClient);
TaskHandle_t wakeTaskHandle = nullptr;
SemaphoreHandle_t wakeMutex = nullptr;

bool wakePending = false;
unsigned long wakeStartMs = 0;
unsigned long lastWakeCheck = 0;
String wakeChatId = "";
String wakeDeviceName = "";
String wakeDeviceIp = "";
int wakeDeviceProbePort = 0;
const unsigned long WAKE_TIMEOUT_MS = 90000;
const unsigned long WAKE_RETRY_MS = 3000;

static void wake_clear_locked() {
  wakePending = false;
  wakeChatId = "";
  wakeDeviceName = "";
  wakeDeviceIp = "";
  wakeDeviceProbePort = 0;
  wakeStartMs = 0;
  lastWakeCheck = 0;
}

static void wake_task(void*) {
  for (;;) {
    String chatId;
    String deviceName;
    String ip;
    int probePort = 0;
    unsigned long startMs = 0;
    unsigned long lastCheckMs = 0;
    bool pending = false;

    if (wakeMutex && xSemaphoreTake(wakeMutex, portMAX_DELAY) == pdTRUE) {
      pending = wakePending;
      chatId = wakeChatId;
      deviceName = wakeDeviceName;
      ip = wakeDeviceIp;
      probePort = wakeDeviceProbePort;
      startMs = wakeStartMs;
      lastCheckMs = lastWakeCheck;
      xSemaphoreGive(wakeMutex);
    }

    if (!pending) {
      vTaskDelay(pdMS_TO_TICKS(250));
      continue;
    }

    if (millis() - lastCheckMs < WAKE_RETRY_MS) {
      vTaskDelay(pdMS_TO_TICKS(250));
      continue;
    }

    if (wake_is_pc_reachable(ip, probePort)) {
      if (wakeMutex && xSemaphoreTake(wakeMutex, portMAX_DELAY) == pdTRUE) {
        wake_clear_locked();
        xSemaphoreGive(wakeMutex);
      }
      wakeNotifyBot.sendMessage(chatId, "🖥 " + deviceName + " is now online! (took "
                                      + String((millis() - startMs) / 1000) + "s)", "");
      Serial.printf("[wake] notify online target=%s chat=%s\n", deviceName.c_str(), chatId.c_str());
      continue;
    }

    if (millis() - startMs >= WAKE_TIMEOUT_MS) {
      if (wakeMutex && xSemaphoreTake(wakeMutex, portMAX_DELAY) == pdTRUE) {
        wake_clear_locked();
        xSemaphoreGive(wakeMutex);
      }
      wakeNotifyBot.sendMessage(chatId, "⚠️ " + deviceName + " did not wake within "
                                      + String(WAKE_TIMEOUT_MS / 1000)
                                      + "s. Check BIOS WoL settings.", "");
      Serial.printf("[wake] notify timeout target=%s chat=%s\n", deviceName.c_str(), chatId.c_str());
      continue;
    }

    if (wakeMutex && xSemaphoreTake(wakeMutex, portMAX_DELAY) == pdTRUE) {
      lastWakeCheck = millis();
      xSemaphoreGive(wakeMutex);
    }
    vTaskDelay(pdMS_TO_TICKS(250));
  }
}

static void wake_ensure_task() {
  if (wakeMutex == nullptr) {
    wakeMutex = xSemaphoreCreateMutex();
  }
  if (wakeTaskHandle == nullptr) {
    wakeNotifyClient.setInsecure();
    xTaskCreatePinnedToCore(wake_task, "wake_task", 4096, nullptr, 1, &wakeTaskHandle, 1);
    Serial.println("[wake] async task started");
  }
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

// ponytail: TCP probe instead of ICMP ping — swap if PC firewall blocks TCP
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
  wake_ensure_task();
  if (wakeMutex && xSemaphoreTake(wakeMutex, portMAX_DELAY) == pdTRUE) {
    wakePending = true;
    wakeStartMs = millis();
    lastWakeCheck = millis();
    wakeChatId = chatId;
    wakeDeviceName = deviceName;
    wakeDeviceIp = ip;
    wakeDeviceProbePort = probePort;
    xSemaphoreGive(wakeMutex);
  }
  Serial.printf("[wake] async pending target=%s chat=%s\n", deviceName.c_str(), chatId.c_str());
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
