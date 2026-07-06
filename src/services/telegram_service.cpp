#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include <Preferences.h>
#include "config.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"
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
  bot.longPoll = 60;
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
  msg += "Current target: ";
  msg += PC_NAME;
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
    bool reachable = wake_is_pc_reachable(PC_IP, PC_TCP_PORT);
    String msg = "✅ ESP32 healthy\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free\n\n";
    msg += "🖥 Target: " + String(PC_NAME) + "\n";
    msg += "   MAC: " + String(PC_MAC) + "\n";
    msg += "   IP: " + String(PC_IP) + "\n";
    msg += "   Status: " + String(reachable ? "online" : "offline / sleeping");
    bot.sendMessage(chatId, msg, "");
    return;
  }

  if (cmd == "/wake") {
    bot.sendMessage(chatId, "⚠️ Confirm wake for " + String(PC_NAME) + "\n\n"
                    "Send /wakeconfirm " + String(PC_NAME) + " to send the WoL packet.", "");
    return;
  }

  if (cmd == "/wakeconfirm") {
    String name = (sp >= 0) ? text.substring(sp + 1) : "";
    name.trim();
    if (name.length() == 0) {
      bot.sendMessage(chatId, "Usage: /wakeconfirm <name>\n\n"
                      "Current target: " + String(PC_NAME), "");
      return;
    }

    if (name != PC_NAME) {
      bot.sendMessage(chatId, "❌ Confirmation mismatch: " + name + "\n\n"
                      "Current target: " + String(PC_NAME) + "\n"
                      "Send /wakeconfirm " + String(PC_NAME) + " to confirm.", "");
      return;
    }

    wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT);
    wake_start_polling(chatId, PC_NAME, PC_IP, PC_TCP_PORT);
    bot.sendMessage(chatId, "⚡ Wake signal sent to " + String(PC_NAME)
                    + " — waiting up to 90s for PC to respond...", "");
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
    bot.longPoll = 60;
    uint32_t pollStart = millis();
    Serial.printf("[telegram] getUpdates mode=idle offset=%ld longPoll=%d\n",
                  telegramOffset,
                  bot.longPoll);
    int newCount = bot.getUpdates(telegramOffset);
    Serial.printf("[telegram] getUpdates done mode=idle updates=%d elapsed=%lums next=%ld\n",
                  newCount,
                  millis() - pollStart,
                  bot.last_message_received + 1);

    if (newCount >= 0) {
      lastPollOkMs = millis();
      if (pollFailStreak > 0) {
        log_event("info", "telegram", "poll_recovered", "getUpdates recovered");
      }
      pollFailStreak = 0;
      pollFailAlerted = false;
    } else {
      pollFailCount++;
      pollFailStreak++;
      if (!pollFailAlerted && pollFailStreak >= 3) {
        pollFailAlerted = true;
        log_event("warn", "telegram", "poll_fail",
                  String("getUpdates failing streak=" + String(pollFailStreak) +
                         " total=" + String(pollFailCount)).c_str());
      }
    }

    for (int i = 0; i < newCount; i++) {
      handleCommand(String(bot.messages[i].chat_id),
                    String(bot.messages[i].text));
    }
    if (newCount > 0) {
      telegramOffset = bot.last_message_received + 1;
      telegramPrefs.putLong("offset", telegramOffset);
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
