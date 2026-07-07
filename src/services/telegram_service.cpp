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

static String fmt_dur(unsigned long s) {
  if (s == 0) return "0s";
  if (s < 60) return String(s) + "s";
  if (s < 3600) return String(s / 60) + "m " + String(s % 60) + "s";
  if (s < 86400) return String(s / 3600) + "h " + String((s % 3600) / 60) + "m";
  return String(s / 86400) + "d " + String((s % 86400) / 3600) + "h";
}

static String menuText() {
  String msg = "<b>ESP32 PC Remote — commands</b>\n\n";
  msg += "/help     Show this menu\n";
  msg += "/ping     Device diagnostics\n";
  msg += "/status   ESP32 + target state\n";
  msg += "/wake     Power on the PC\n";
  msg += "/reboot   Restart the ESP32\n\n";
  msg += "Target: <b>";
  msg += PC_NAME;
  msg += "</b>";
  return msg;
}

static void handleCommand(String chatId, String text) {
  text.trim();

  // extract command prefix (first word) for case-insensitive matching
  int sp = text.indexOf(' ');
  String cmd = (sp >= 0) ? text.substring(0, sp) : text;
  cmd.toLowerCase();

  if (cmd == "/start" || cmd == "/help") {
    bot.sendMessage(chatId, menuText(), "HTML");
    return;
  }

  if (cmd == "/ping") {
    String msg = "<b>ESP32 diagnostics</b>\n\n";
    msg += "Wi-Fi:     " + String(WiFi.status() == WL_CONNECTED
                                  ? "connected" : "disconnected");
    if (WiFi.status() == WL_CONNECTED) {
      msg += ", " + String(WiFi.RSSI()) + " dBm";
    }
    msg += "\n";
    msg += "IP:        " + WiFi.localIP().toString() + "\n";
    msg += "Uptime:    " + fmt_dur(millis() / 1000) + "\n";
    msg += "Heap:      " + String(ESP.getFreeHeap() / 1024) + " KB free\n";
    msg += "Reset:     " + String(current_reset_reason()) + "\n";

    bot.sendMessage(chatId, msg, "HTML");
    return;
  }

  if (cmd == "/status") {
    String msg;
    bool wifiUp = WiFi.status() == WL_CONNECTED;
    bool reachable = false;

    if (wifiUp && !wake_is_pending()) {
      reachable = wake_is_pc_reachable(PC_IP, PC_TCP_PORT);
    }

    if (wifiUp) {
      msg = "ESP32 — connected, " + String(WiFi.RSSI()) + " dBm, "
          + WiFi.localIP().toString() + ", up "
          + fmt_dur(millis() / 1000) + "\n\n";
    } else {
      msg = "<b>ESP32 — disconnected</b>\n\n";
    }

    if (!wifiUp) {
      msg += "Target: " + String(PC_NAME) + " — can't check\n";
    } else if (wake_is_pending()) {
      msg += "Target: <b>" + String(PC_NAME) + "</b> — waking... ("
          + String(wake_pending_elapsed_seconds()) + "s)\n";
    } else if (reachable) {
      wake_mark_online_seen();
      msg += "Target: <b>" + String(PC_NAME) + "</b> — online (seen just now)\n";
    } else {
      msg += "Target: <b>" + String(PC_NAME) + "</b> — not reachable\n";
      unsigned long age = wake_last_online_age_seconds();
      if (age > 0) {
        msg += "Seen:   " + fmt_dur(age) + " ago\n";
      }
    }

    if (!wake_is_pending()) {
      String lastWake = wake_last_result();
      if (lastWake.length() > 0) {
        msg += "Wake:   ";
        if (lastWake == "success")       msg += "succeeded";
        else if (lastWake == "timeout")  msg += "timed out";
        else                             msg += lastWake;

        unsigned long age = wake_last_result_age_seconds();
        if (age > 0) {
          msg += " " + fmt_dur(age) + " ago";
        }
        msg += "\n";
      }
    }

    bot.sendMessage(chatId, msg, "HTML");
    return;
  }

  if (cmd == "/wake") {
    DynamicJsonDocument payload(1024);
    payload["chat_id"] = chatId;
    payload["parse_mode"] = "HTML";
    payload["text"] = "Wake <b>" + String(PC_NAME) + "</b>?";
    JsonObject markup = payload.createNestedObject("reply_markup");
    JsonArray rows = markup.createNestedArray("inline_keyboard");
    JsonArray row = rows.createNestedArray();
    JsonObject yes = row.createNestedObject();
    yes["text"] = "✅ Yes";
    yes["callback_data"] = "wake_confirm";
    JsonObject no = row.createNestedObject();
    no["text"] = "❌ No";
    no["callback_data"] = "wake_cancel";
    bot.sendPostToTelegram(bot.buildCommand("sendMessage"), payload.as<JsonObject>());
    client.stop();
    return;
  }

  if (cmd == "/reboot") {
    bot.sendMessage(chatId, "Restarting ESP32...", "HTML");
    telegramRebootPending = true;
    return;
  }

  // unknown command — show help hint
  bot.sendMessage(chatId, "Unknown command. Try /help for available commands.", "HTML");
}

static void handleCallback(const telegramMessage& msg) {
  if (msg.text == "wake_confirm") {
    bot.answerCallbackQuery(msg.query_id, "", false, "", 0);
    wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT);
    wake_start_polling(msg.chat_id, PC_NAME, PC_IP, PC_TCP_PORT);
    bot.sendMessage(msg.chat_id, "Wake sent to <b>" + String(PC_NAME) + "</b> — waiting up to 90s...", "HTML");
  } else if (msg.text == "wake_cancel") {
    bot.answerCallbackQuery(msg.query_id, "Cancelled", false, "", 0);
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
    // ponytail: close the socket after every poll; UniversalTelegramBot keeps
    // it open on success, which breaks the next request on reused TLS state.
    client.stop();
    Serial.printf("[telegram] getUpdates done mode=idle updates=%d elapsed=%lums next=%ld\n",
                  newCount,
                  millis() - pollStart,
                  bot.last_message_received + 1);

    if (newCount < 0) {
      Serial.printf("[telegram] getUpdates failed\n");
    }

    for (int i = 0; i < newCount; i++) {
      if (bot.messages[i].type == "callback_query") {
        handleCallback(bot.messages[i]);
      } else {
        handleCommand(String(bot.messages[i].chat_id),
                      String(bot.messages[i].text));
      }
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
