#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include "config.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"
#include "src/services/device_service.h"

WiFiClientSecure client;
UniversalTelegramBot bot(BOT_TOKEN, client);
unsigned long lastPoll = 0;
const unsigned long POLL_MS = 1500;

void telegram_setup() {
  // ponytail: setInsecure for local/MVP; add root CA cert for production use
  client.setInsecure();
}

static String menuText() {
  String msg = "🤖 *suibot commands*\n\n";
  msg += "/start /help — Show this menu\n";
  msg += "/ping — Check bot health & diagnostics\n";
  msg += "/status — ESP32 health + target PC state\n";
  msg += "/wake — Wake the selected PC\n";
  msg += "/devices — List known machines\n";
  msg += "/target <name> — Switch active PC\n";
  msg += "/reboot — Restart the ESP32\n\n";
  msg += "Current target: " + device_active_name();
  return msg;
}

static void handleCommand(String chatId, String text) {
  text.trim();

  // extract command prefix (first word) for case-insensitive matching
  int sp = text.indexOf(' ');
  String cmd = (sp >= 0) ? text.substring(0, sp) : text;
  cmd.toLowerCase();

  if (cmd == "/start" || cmd == "/help") {
    bot.sendMessage(chatId, menuText(), "Markdown");
    return;
  }

  if (cmd == "/ping") {
    String msg = "🤖 Bot alive\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free";
    bot.sendMessage(chatId, msg, "");
    return;
  }

  if (cmd == "/status") {
    Device dev = device_get_active();
    bool reachable = wake_is_pc_reachable(dev.ip, dev.probePort);
    String msg = "✅ ESP32 healthy\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free\n\n";
    msg += "🖥 Target: " + dev.name + "\n";
    msg += "   MAC: " + dev.mac + "\n";
    msg += "   IP: " + dev.ip + "\n";
    msg += "   Status: " + String(reachable ? "online" : "offline / sleeping");
    bot.sendMessage(chatId, msg, "");
    return;
  }

  if (cmd == "/wake") {
    Device dev = device_get_active();
    wake_send_magic(dev.mac, dev.bcast, dev.wolPort);
    wake_start_polling(chatId);
    bot.sendMessage(chatId, "⚡ Wake signal sent to " + dev.name
                    + " — waiting up to " + String(wake_timeout_seconds())
                    + "s for PC to respond...", "");
    return;
  }

  if (cmd == "/devices") {
    String msg = "📋 Known machines:\n";
    int activeIdx = device_active_index();
    for (int i = 0; i < device_count(); i++) {
      const Device& d = device_get(i);
      msg += (i == activeIdx ? "👉 " : "   ") + d.name;
      msg += " — " + d.ip + "\n";
    }
    bot.sendMessage(chatId, msg, "");
    return;
  }

  if (cmd == "/target") {
    String name = (sp >= 0) ? text.substring(sp + 1) : "";
    name.trim();
    if (name.length() == 0) {
      bot.sendMessage(chatId, "Usage: /target <name>\n\n"
                      "Current target: " + device_active_name(), "");
      return;
    }
    int idx = device_find(name);
    if (idx >= 0) {
      device_set_active(idx);
      bot.sendMessage(chatId, "✅ Target switched to " + name, "");
    } else {
      String msg = "❌ Unknown target: " + name + "\n\nAvailable: ";
      for (int i = 0; i < device_count(); i++) {
        if (i > 0) msg += ", ";
        msg += device_get(i).name;
      }
      bot.sendMessage(chatId, msg, "");
    }
    return;
  }

  if (cmd == "/reboot") {
    bot.sendMessage(chatId, "🔄 Rebooting ESP32...", "");
    delay(100);
    ESP.restart();
    return;
  }

  // unknown command — show help hint
  bot.sendMessage(chatId, "Unknown command. Type /help for available commands.", "");
}

void telegram_poll() {
  if (millis() - lastPoll >= POLL_MS) {
    int newCount = bot.getUpdates(bot.last_message_received + 1);
    for (int i = 0; i < newCount; i++) {
      handleCommand(String(bot.messages[i].chat_id),
                    String(bot.messages[i].text));
    }
    lastPoll = millis();
  }

  // wake polling — auto-reply once PC comes online or times out
  Device dev = device_get_active();
  int w = wake_tick(dev.ip, dev.probePort);
  if (w == 1) {
    bot.sendMessage(wake_chat_id(), "🖥 " + dev.name + " is now online! (took "
                    + String(wake_elapsed_seconds()) + "s)", "");
  } else if (w == -1) {
    bot.sendMessage(wake_chat_id(), "⚠️ " + dev.name + " did not wake within "
                    + String(wake_timeout_seconds())
                    + "s. Check BIOS WoL settings.", "");
  }
}
