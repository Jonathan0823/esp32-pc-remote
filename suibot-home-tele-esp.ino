/*
 * ESP32 Telegram Bot — suibot-home-tele-esp
 *
 * Dependencies (install via Arduino CLI):
 *   arduino-cli core install esp32:esp32
 *   arduino-cli lib install "UniversalTelegramBot"
 *
 * Setup:
 *   1. Copy config.example.h → config.h
 *   2. Fill in WIFI_SSID, WIFI_PASS, BOT_TOKEN
 *   3. Compile & upload:
 *      arduino-cli compile --fqbn esp32:esp32:esp32 .
 *      arduino-cli upload --fqbn esp32:esp32:esp32 -p /dev/ttyUSB0 .
 */

#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <UniversalTelegramBot.h>
#include "config.h"

WiFiClientSecure client;
UniversalTelegramBot bot(BOT_TOKEN, client);

unsigned long lastPoll = 0;
const unsigned long POLL_MS = 1500;

void setup() {
  Serial.begin(115200);
  Serial.println("\n=== ESP32 Telegram Bot ===");

  // ponytail: setInsecure for local/MVP; add root CA cert for production use
  client.setInsecure();

  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  Serial.print("Connecting to Wi-Fi");
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    Serial.print(".");
  }
  Serial.println("\nWi-Fi connected");
  Serial.print("IP: ");
  Serial.println(WiFi.localIP());
}

void loop() {
  // T004: reconnect on Wi-Fi loss
  if (WiFi.status() != WL_CONNECTED) {
    Serial.println("Wi-Fi lost — reconnecting...");
    WiFi.reconnect();
    delay(2000);
    return;
  }

  if (millis() - lastPoll >= POLL_MS) {
    int newCount = bot.getUpdates(bot.last_message_received + 1);
    for (int i = 0; i < newCount; i++) {
      handleCommand(
        String(bot.messages[i].chat_id),
        String(bot.messages[i].text)
      );
    }
    lastPoll = millis();
  }
}

void handleCommand(String chatId, String text) {
  text.trim();
  if (text == "/ping") {
    bot.sendMessage(chatId, "pong", "");
    return;
  }
  if (text == "/status") {
    String msg = "✅ Bot alive\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s";
    bot.sendMessage(chatId, msg, "");
    return;
  }
  // unknown command — ignore
}
