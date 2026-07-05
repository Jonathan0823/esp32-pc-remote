#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include "config.h"
#include "src/services/telegram_service.h"
#include "src/services/wake_service.h"

WiFiClientSecure client;
UniversalTelegramBot bot(BOT_TOKEN, client);
unsigned long lastPoll = 0;
const unsigned long POLL_MS = 1500;

void telegram_setup() {
  // ponytail: setInsecure for local/MVP; add root CA cert for production use
  client.setInsecure();
}

static void handleCommand(String chatId, String text) {
  text.trim();
  if (text == "/ping") {
    bot.sendMessage(chatId, "pong", "");
    return;
  }
  if (text == "/wake") {
    wake_send_magic();
    wake_start_polling(chatId);
    bot.sendMessage(chatId, "⚡ Wake signal sent — waiting up to "
                    + String(wake_timeout_seconds())
                    + "s for PC to respond...", "");
    return;
  }
  if (text == "/status") {
    String msg = "✅ Bot alive\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED
                                 ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "🖥 " + String(PC_NAME) + "\n";
    msg += "   MAC: " + String(PC_MAC) + "\n";
    msg += "   Target: " + String(PC_IP) + "\n";
    msg += "   Status: " + String(wake_is_pc_reachable()
                                  ? "online" : "offline / sleeping");
    bot.sendMessage(chatId, msg, "");
    return;
  }
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
  int w = wake_tick();
  if (w == 1) {
    bot.sendMessage(wake_chat_id(), "🖥 PC is now online! (took "
                    + String(wake_elapsed_seconds()) + "s)", "");
  } else if (w == -1) {
    bot.sendMessage(wake_chat_id(), "⚠️ PC did not wake within "
                    + String(wake_timeout_seconds())
                    + "s. Check BIOS WoL settings.", "");
  }
}
