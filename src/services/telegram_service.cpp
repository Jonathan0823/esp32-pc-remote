#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include <Preferences.h>
#include "config.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"
#include "src/services/device_service.h"
#include "src/services/log_service.h"

// Declared in esp32-pc-remote.ino
extern const char* current_reset_reason();

WiFiClientSecure client;
UniversalTelegramBot bot(BOT_TOKEN, client);
Preferences telegramPrefs;
long telegramOffset = 1;
bool telegramRebootPending = false;
unsigned long lastPoll = 0;
const unsigned long POLL_MS = 500;

// Diagnostic trackers
static unsigned long lastPollOkMs = 0;
static int pollFailCount = 0;
static int pollFailStreak = 0;
static bool pollFailAlerted = false;

void telegram_setup() {
  // ponytail: setInsecure for local/MVP; add root CA cert for production use
  client.setInsecure();
  bot.longPoll = 30;
  bot.waitForResponse = 250;
  telegramPrefs.begin("telegram", false);
  telegramOffset = telegramPrefs.getLong("offset", 1);
  Serial.printf("[telegram] setup longPoll=%d wait=%u offset=%ld\n",
                bot.longPoll,
                bot.waitForResponse,
                telegramOffset);
}

static String menuText() {
  String msg = "🤖 *ESP32 PC Remote commands*\n\n";
  msg += "/start /help — Show this menu\n";
  msg += "/ping — Check bot health & diagnostics\n";
  msg += "/status — ESP32 health + target PC state\n";
  msg += "/wake — Ask for confirmation to wake the selected PC\n";
  msg += "/wakeconfirm <name> — Confirm and send WoL\n";
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
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free\n";
    msg += "🔄 Reset: " + String(current_reset_reason()) + "\n";
    msg += "📬 Poll OK: " + String(lastPollOkMs > 0
                                   ? String((millis() - lastPollOkMs) / 1000) + "s ago"
                                   : "never") + "\n";
    msg += "⚠️ Poll fails: " + String(pollFailCount);
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
    bot.sendMessage(chatId, "⚠️ Confirm wake for " + dev.name + "\n\n"
                    "Send /wakeconfirm " + dev.name + " to send the WoL packet.", "");
    return;
  }

  if (cmd == "/wakeconfirm") {
    String name = (sp >= 0) ? text.substring(sp + 1) : "";
    name.trim();
    if (name.length() == 0) {
      bot.sendMessage(chatId, "Usage: /wakeconfirm <name>\n\n"
                      "Current target: " + device_active_name(), "");
      return;
    }

    String activeName = device_active_name();
    if (name != activeName) {
      bot.sendMessage(chatId, "❌ Confirmation mismatch: " + name + "\n\n"
                      "Current target: " + activeName + "\n"
                      "Send /wakeconfirm " + activeName + " to confirm.", "");
      return;
    }

    Device dev = device_get_active();
    wake_send_magic(dev.mac, dev.bcast, dev.wolPort);
    wake_start_polling(chatId, dev.name, dev.ip, dev.probePort);
    bot.sendMessage(chatId, "⚡ Wake signal sent to " + dev.name
                    + " — waiting up to " + String(wake_timeout_seconds())
                    + "s for PC to respond...", "");
    return;
  }

  if (cmd == "/reboot") {
    bot.sendMessage(chatId, "🔄 Rebooting ESP32...", "");
    telegramRebootPending = true;
    return;
  }

  // unknown command — show help hint
  bot.sendMessage(chatId, "Unknown command. Type /help for available commands.", "");
}

void telegram_poll() {
  if (millis() - lastPoll >= POLL_MS) {
    bot.longPoll = 30;
    uint32_t pollStart = millis();
    Serial.printf("[telegram] getUpdates mode=idle offset=%ld longPoll=%d\n",
                  telegramOffset,
                  bot.longPoll);
    int newCount = bot.getUpdates(telegramOffset);
    uint32_t elapsed = millis() - pollStart;
    Serial.printf("[telegram] getUpdates done mode=idle updates=%d elapsed=%lums next=%ld\n",
                  newCount,
                  elapsed,
                  bot.last_message_received + 1);

    if (newCount >= 0) {
      // Successful poll
      lastPollOkMs = millis();
      if (pollFailStreak > 0) {
        log_event("info", "telegram", "poll_recovered", "getUpdates recovered");
      }
      pollFailStreak = 0;
      pollFailAlerted = false;
      if (newCount > 0) {
        // New messages received — process them
        for (int i = 0; i < newCount; i++) {
          handleCommand(String(bot.messages[i].chat_id),
                        String(bot.messages[i].text));
        }
        telegramOffset = bot.last_message_received + 1;
        telegramPrefs.putLong("offset", telegramOffset);
      }
    } else {
      // Poll error
      pollFailCount++;
      pollFailStreak++;
      if (!pollFailAlerted && pollFailStreak >= 3) {
        pollFailAlerted = true;
        log_event("warn", "telegram", "poll_fail",
                  String("getUpdates failing streak=" + String(pollFailStreak) +
                         " total=" + String(pollFailCount) +
                         " elapsed=" + String(elapsed) + "ms").c_str());
      }
    }

    if (telegramRebootPending) {
      telegramPrefs.putLong("offset", telegramOffset);
      Serial.printf("[telegram] reboot pending; offset=%ld\n", telegramOffset);
      delay(100);
      ESP.restart();
    }
    lastPoll = millis();
  }
}
