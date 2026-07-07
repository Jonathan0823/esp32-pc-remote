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
  msg += "/wake — Wake the selected PC (inline confirmation)\n";
  msg += "/reboot — Restart the ESP32\n\n";
  msg += "Current target: ";
  msg += PC_NAME;
  return msg;
}

static void handleCommand(String chatId, String text) {
  text.trim();
  Serial.printf("[telegram] handleCommand chat=%s text=%s\n", chatId.c_str(), text.c_str());

  // extract command prefix (first word) for case-insensitive matching
  int sp = text.indexOf(' ');
  String cmd = (sp >= 0) ? text.substring(0, sp) : text;
  cmd.toLowerCase();

  if (cmd == "/start" || cmd == "/help") {
    Serial.println("[telegram] /help reply start");
    bot.sendMessage(chatId, menuText(), "Markdown");
    Serial.println("[telegram] /help reply done");
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

    Serial.println("[telegram] /ping reply start");
    bot.sendMessage(chatId, msg, "");
    Serial.println("[telegram] /ping reply done");
    return;
  }

  if (cmd == "/status") {
    Serial.println("[telegram] /status probe start");
    bool reachable = wake_is_pc_reachable(PC_IP, PC_TCP_PORT);
    Serial.printf("[telegram] /status probe done reachable=%d\n", reachable);
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
    Serial.println("[telegram] /status reply start");
    bot.sendMessage(chatId, msg, "");
    Serial.println("[telegram] /status reply done");
    return;
  }

  if (cmd == "/wake") {
    DynamicJsonDocument payload(1024);
    payload["chat_id"] = chatId;
    payload["text"] = "Wake " + String(PC_NAME) + "?";
    JsonObject markup = payload.createNestedObject("reply_markup");
    JsonArray rows = markup.createNestedArray("inline_keyboard");
    JsonArray row = rows.createNestedArray();
    JsonObject yes = row.createNestedObject();
    yes["text"] = "✅ Yes";
    yes["callback_data"] = "wake_confirm";
    JsonObject no = row.createNestedObject();
    no["text"] = "❌ No";
    no["callback_data"] = "wake_cancel";
    Serial.println("[telegram] /wake reply start");
    bot.sendPostToTelegram(bot.buildCommand("sendMessage"), payload.as<JsonObject>());
    client.stop();
    Serial.println("[telegram] /wake reply done");
    return;
  }

  if (cmd == "/reboot") {
    Serial.println("[telegram] /reboot reply start");
    bot.sendMessage(chatId, "🔄 Rebooting ESP32...", "");
    Serial.println("[telegram] /reboot reply done");
    telegramRebootPending = true;
    return;
  }

  // unknown command — show help hint
  Serial.println("[telegram] unknown command reply start");
  bot.sendMessage(chatId, "Unknown command. Type /help for available commands.", "");
  Serial.println("[telegram] unknown command reply done");
}

static void handleCallback(const telegramMessage& msg) {
  Serial.printf("[telegram] handleCallback chat=%s data=%s\n", msg.chat_id.c_str(), msg.text.c_str());
  if (msg.text == "wake_confirm") {
    Serial.println("[telegram] callback confirm start");
    bot.answerCallbackQuery(msg.query_id, "", false, "", 0);
    wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT);
    wake_start_polling(msg.chat_id, PC_NAME, PC_IP, PC_TCP_PORT);
    bot.sendMessage(msg.chat_id, "⚡ Wake signal sent to " + String(PC_NAME)
                    + " — waiting up to 90s for PC to respond...", "");
    Serial.println("[telegram] callback confirm done");
  } else if (msg.text == "wake_cancel") {
    Serial.println("[telegram] callback cancel start");
    bot.answerCallbackQuery(msg.query_id, "Cancelled", false, "", 0);
    Serial.println("[telegram] callback cancel done");
  }
}

void telegram_poll() {
  if (millis() - lastPoll >= POLL_MS) {
    // ponytail: while wake is pending, use 5s polls so wake detection
    // isn't delayed by the 60s long-poll
    bot.longPoll = wake_is_pending() ? 5 : 60;
    uint32_t pollStart = millis();
    Serial.printf("[telegram] getUpdates mode=idle offset=%ld longPoll=%d\n",
                  telegramOffset,
                  bot.longPoll);
    int newCount = bot.getUpdates(telegramOffset);
    // ponytail: close the socket after each poll; keep TLS state from going stale.
    client.stop();
    Serial.println("[telegram] client stopped after getUpdates");
    Serial.printf("[telegram] getUpdates done mode=idle updates=%d elapsed=%lums next=%ld\n",
                  newCount,
                  millis() - pollStart,
                  bot.last_message_received + 1);

    if (newCount < 0) {
      Serial.printf("[telegram] getUpdates failed\n");
    }

    for (int i = 0; i < newCount; i++) {
      Serial.printf("[telegram] update[%d] type=%s text=%s\n",
                    i,
                    bot.messages[i].type.c_str(),
                    bot.messages[i].text.c_str());
      if (bot.messages[i].type == "callback_query") {
        handleCallback(bot.messages[i]);
      } else {
        handleCommand(String(bot.messages[i].chat_id),
                      String(bot.messages[i].text));
      }
      Serial.printf("[telegram] update[%d] handled\n", i);
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
