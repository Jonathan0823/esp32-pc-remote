#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <esp_task_wdt.h>
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

static bool telegram_send_once(const char* label,
                               const String& command,
                               JsonObject payload) {
  // ponytail: no client.stop() before send — library reconnects if needed,
  //           and the poll already kept the connection alive.
  String response = bot.sendPostToTelegram(command, payload);

  if (response.length() == 0) {
    log_print("[telegram] %s send failed: empty response\n", label);
    log_event("error", "telegram", label, "empty response from Telegram");
    return false;
  }

  DynamicJsonDocument doc(512);
  DeserializationError error = deserializeJson(doc, response);
  if (error) {
    log_print("[telegram] %s send failed: %s\n", label, error.c_str());
    log_event("error", "telegram", label, error.c_str());
    return false;
  }

  bool ok = doc["ok"] | false;
  if (!ok) {
    const char* desc = doc["description"] | "Telegram API returned ok=false";
    log_print("[telegram] %s send not_ok: %s\n", label, desc);
    log_event("error", "telegram", label, desc);
  } else {
    log_print("[telegram] %s send ok\n", label);
  }
  return ok;
}

bool telegram_send_json_once(const String& command, JsonObject payload, const char* label) {
  return telegram_send_once(label, command, payload);
}

bool telegram_send_text_once(const String& chatId, const String& text, const String& parse_mode) {
  DynamicJsonDocument payload(1024);
  payload["chat_id"] = chatId;
  payload["text"] = text;
  if (parse_mode.length() > 0) {
    payload["parse_mode"] = parse_mode;
  }
  return telegram_send_once("sendMessage", bot.buildCommand("sendMessage"), payload.as<JsonObject>());
}

bool telegram_send_callback_answer_once(const String& query_id,
                                        const String& text,
                                        bool show_alert,
                                        const String& url,
                                        int cache_time) {
  DynamicJsonDocument payload(256);
  payload["callback_query_id"] = query_id;
  payload["show_alert"] = show_alert;
  payload["cache_time"] = cache_time;

  if (text.length() > 0) payload["text"] = text;
  if (url.length() > 0) payload["url"] = url;

  return telegram_send_once("answerCallbackQuery",
                            bot.buildCommand("answerCallbackQuery"),
                            payload.as<JsonObject>());
}

void telegram_setup() {
  // ponytail: setInsecure for local/MVP; add root CA cert for production use
  client.setInsecure();
  bot.longPoll = 60;
  bot.waitForResponse = 1500;
  telegramPrefs.begin("telegram", false);
  telegramOffset = telegramPrefs.getLong("offset", 1);
  log_print("[telegram] setup longPoll=%d wait=%u offset=%ld\n",
                bot.longPoll,
                bot.waitForResponse,
                telegramOffset);
}

static String menuText() {
  String msg = "🤖 <b>ESP32 PC Remote commands</b>\n\n";
  msg += "/start /help — Show this menu\n";
  msg += "/ping — Check bot health &amp; diagnostics\n";
  msg += "/status — ESP32 health + target PC state\n";
  msg += "/wake — Wake the selected PC (inline confirmation)\n";
  msg += "/reboot — Restart the ESP32\n\n";
  msg += "Current target: ";
  msg += PC_NAME;
  return msg;
}

static void handleCommand(String chatId, String text) {
  text.trim();
  log_print("[telegram] handleCommand chat=%s text=%s\n", chatId.c_str(), text.c_str());

  // extract command prefix (first word) for case-insensitive matching
  int sp = text.indexOf(' ');
  String cmd = (sp >= 0) ? text.substring(0, sp) : text;
  cmd.toLowerCase();

  if (cmd == "/start" || cmd == "/help") {
    log_print("[telegram] /help reply start\n");
    telegram_send_text_once(chatId, menuText(), "HTML");
    log_print("[telegram] /help reply done\n");
    return;
  }

  if (cmd == "/ping") {
    String msg = "🤖 <b>Bot alive</b>\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free\n";
    msg += "🔄 Reset: " + String(current_reset_reason()) + "\n";

    log_print("[telegram] /ping reply start\n");
    telegram_send_text_once(chatId, msg, "HTML");
    log_print("[telegram] /ping reply done\n");
    return;
  }

  if (cmd == "/status") {
    log_print("[telegram] /status probe start\n");
    bool reachable = wake_is_pc_reachable(PC_IP, PC_TCP_PORT);
    log_print("[telegram] /status probe done reachable=%d\n", reachable);
    String msg = "✅ <b>ESP32 healthy</b>\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "💾 Heap: " + String(ESP.getFreeHeap() / 1024) + " KB free\n\n";
    msg += "🖥 <b>Target: " + String(PC_NAME) + "</b>\n";
    msg += "   MAC: " + String(PC_MAC) + "\n";
    msg += "   IP: " + String(PC_IP) + "\n";
    msg += "   Status: " + String(reachable ? "online" : "offline / sleeping");
    log_print("[telegram] /status reply start\n");
    telegram_send_text_once(chatId, msg, "HTML");
    log_print("[telegram] /status reply done\n");
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
    log_print("[telegram] /wake reply start\n");
    telegram_send_json_once(bot.buildCommand("sendMessage"), payload.as<JsonObject>(), "sendMessage");
    log_print("[telegram] /wake reply done\n");
    return;
  }

  if (cmd == "/reboot") {
    log_print("[telegram] /reboot reply start\n");
    telegram_send_text_once(chatId, "🔄 Rebooting ESP32...", "");
    log_print("[telegram] /reboot reply done\n");
    telegramRebootPending = true;
    return;
  }

  // unknown command — show help hint
  log_print("[telegram] unknown command reply start\n");
  telegram_send_text_once(chatId, "Unknown command. Type /help for available commands.", "");
  log_print("[telegram] unknown command reply done\n");
}

static void handleCallback(const telegramMessage& msg) {
  log_print("[telegram] handleCallback chat=%s data=%s\n", msg.chat_id.c_str(), msg.text.c_str());
  if (msg.text == "wake_confirm") {
    log_print("[telegram] callback confirm start\n");
    telegram_send_callback_answer_once(msg.query_id, "", false, "", 0);
    wake_send_magic(PC_MAC, WOL_BCAST, WOL_PORT);
    wake_start_polling(msg.chat_id, PC_NAME, PC_IP, PC_TCP_PORT);
    telegram_send_text_once(msg.chat_id, "⚡ Wake signal sent to " + String(PC_NAME)
                    + " — waiting up to 90s for PC to respond...", "");
    log_print("[telegram] callback confirm done\n");
  } else if (msg.text == "wake_cancel") {
    log_print("[telegram] callback cancel start\n");
    telegram_send_callback_answer_once(msg.query_id, "Cancelled", false, "", 0);
    log_print("[telegram] callback cancel done\n");
  }
}

void telegram_poll() {
  if (millis() - lastPoll >= POLL_MS) {
    // ponytail: keep the TLS connection alive if still fresh (<120s idle).
    //           Reuse avoids the ~1-3s handshake on every poll.
    if (!client.connected() || millis() - lastPoll > 120000) {
      client.stop();
    }
    // Reset WDT before blocking getUpdates call (can block up to 60s).
    esp_task_wdt_reset();

    // ponytail: while wake is pending, use 5s polls so wake detection
    // isn't delayed by the 60s long-poll
    bot.longPoll = wake_is_pending() ? 5 : 60;
    uint32_t pollStart = millis();
    log_print("[telegram] getUpdates mode=idle offset=%ld longPoll=%d\n",
                  telegramOffset,
                  bot.longPoll);
    int newCount = bot.getUpdates(telegramOffset);
    // ponytail: reset longPoll after polling so send helpers don't inherit the 60s poll timeout.
    bot.longPoll = 0;
    log_print("[telegram] getUpdates done mode=idle updates=%d elapsed=%lums next=%ld\n",
                  newCount,
                  millis() - pollStart,
                  bot.last_message_received + 1);

    if (newCount < 0) {
      log_print("[telegram] getUpdates failed\n");
      log_event("error", "telegram", "getUpdates", "getUpdates failed");
    }

    for (int i = 0; i < newCount; i++) {
      log_print("[telegram] update[%d] type=%s text=%s\n",
                    i,
                    bot.messages[i].type.c_str(),
                    bot.messages[i].text.c_str());
      if (bot.messages[i].type == "callback_query") {
        handleCallback(bot.messages[i]);
      } else {
        handleCommand(String(bot.messages[i].chat_id),
                      String(bot.messages[i].text));
      }
      log_print("[telegram] update[%d] handled\n", i);
    }
    if (newCount > 0) {
      telegramOffset = bot.last_message_received + 1;
      telegramPrefs.putLong("offset", telegramOffset);
    }
    if (telegramRebootPending) {
      telegramPrefs.putLong("offset", telegramOffset);
      log_print("[telegram] reboot pending; offset=%ld\n", telegramOffset);
      delay(100);
      ESP.restart();
    }
    lastPoll = millis();
  }
}
