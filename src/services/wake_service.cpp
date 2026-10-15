#include <WiFi.h>
#include <WiFiUdp.h>
#include <esp_system.h>
#include "config.h"
#include "src/services/wake_service.h"
#include "src/services/telegram_service.h"
#include "src/services/log_service.h"
#include "src/services/indicator_service.h"
#include "src/services/wol_packet.h"

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

bool wake_is_pending() {
  return wakePending;
}

void wake_poll() {
  if (!wakePending) return;
  if (WiFi.status() != WL_CONNECTED) return;
  if (millis() - lastWakeCheck < WAKE_RETRY_MS) return;

  if (wake_is_pc_reachable(wakeDeviceIp, wakeDeviceProbePort)) {
    telegram_send_text_once(wakeChatId,
                            "🖥 " + wakeDeviceName + " is now online! (took "
                            + String((millis() - wakeStartMs) / 1000) + "s)",
                            "");
    wake_clear();
    return;
  }

  if (millis() - wakeStartMs >= WAKE_TIMEOUT_MS) {
    String warnMsg = "PC did not wake within " + String(WAKE_TIMEOUT_MS / 1000) + "s";
    log_event("warn", "wake", "timeout", warnMsg.c_str());
    telegram_send_text_once(wakeChatId,
                            "⚠️ " + wakeDeviceName + " did not wake within "
                            + String(WAKE_TIMEOUT_MS / 1000)
                            + "s. Check BIOS WoL settings.",
                            "");
    wake_clear();
    return;
  }

  lastWakeCheck = millis();
}

String wake_new_id() {
  char id[9];
  snprintf(id, sizeof(id), "%08lx", static_cast<unsigned long>(esp_random()));
  return String(id);
}

bool wake_send_magic(const String& mac, const String& bcast, int port,
                     const char* source, const String& wakeId) {
  const char* reason = nullptr;
  bool sent = false;
  if (port < 1 || port > 65535) {
    reason = "invalid_port";
  } else {
    byte packet[102];
    if (!wol_build_magic_packet(mac.c_str(), packet)) {
      reason = "invalid_mac";
    } else {
      IPAddress bcastAddr;
      if (!bcastAddr.fromString(bcast)) {
        reason = "invalid_broadcast";
      } else {
        WiFiUDP udp;
        if (!udp.beginPacket(bcastAddr, port)) {
          reason = "udp_begin_failed";
        } else if (udp.write(packet, sizeof(packet)) != sizeof(packet)) {
          udp.stop();
          reason = "udp_write_failed";
        } else {
          sent = udp.endPacket() == 1;
          udp.stop();
          if (!sent) reason = "udp_send_failed";
        }
      }
    }
  }

  log_wake_event(source, "packet_submission", wakeId.c_str(), reason, true, sent);
  indicator_wake_result(source, sent);
  return sent;
}

bool wake_is_pc_reachable(const String& ip, int port) {
  WiFiClient probe;
  uint32_t start = millis();
  log_print("[wake] probe %s:%d start\n", ip.c_str(), port);
  // ponytail: local network TCP connect should either succeed (<10ms)
  // or fail with RST (<100ms); 200ms is a safe ceiling
  bool r = probe.connect(ip.c_str(), port, 200);
  probe.stop();
  log_print("[wake] probe %s:%d %s in %lums\n",
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
  log_print("[wake] pending target=%s chat=%s\n", deviceName.c_str(), chatId.c_str());
}
