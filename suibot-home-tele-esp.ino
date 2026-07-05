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
#include <WiFiUdp.h>
#include <UniversalTelegramBot.h>
#include "config.h"

WiFiClientSecure client;
UniversalTelegramBot bot(BOT_TOKEN, client);

unsigned long lastPoll = 0;
const unsigned long POLL_MS = 1500;

WiFiUDP udp;

void sendWoL() {
  byte mac[6];
  sscanf(PC_MAC, "%hhx:%hhx:%hhx:%hhx:%hhx:%hhx", &mac[0], &mac[1], &mac[2], &mac[3], &mac[4], &mac[5]);
  byte packet[102];
  memset(packet, 0xFF, 6);
  for (int i = 0; i < 16; i++) memcpy(&packet[6 + i * 6], mac, 6);
  IPAddress bcast;
  bcast.fromString(WOL_BCAST);
  udp.beginPacket(bcast, WOL_PORT);
  udp.write(packet, 102);
  udp.endPacket();
  Serial.println("WoL sent to " + String(PC_MAC) + " via " + String(WOL_BCAST) + ":" + String(WOL_PORT));
}

bool isPCReachable() {
  // ponytail: TCP probe to Artemis port; swap to ICMP ping if reliability matters
  WiFiClient probe;
  bool r = probe.connect(PC_IP, 47989, 2000);
  probe.stop();
  return r;
}

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
  if (text == "/wake") {
    sendWoL();
    bot.sendMessage(chatId, "⚡ Magic packet sent — PC should wake soon", "");
    return;
  }
  if (text == "/status") {
    String msg = "✅ Bot alive\n";
    msg += "📶 Wi-Fi: " + String(WiFi.status() == WL_CONNECTED ? "connected" : "disconnected") + "\n";
    msg += "📡 RSSI: " + String(WiFi.RSSI()) + " dBm\n";
    msg += "🌐 IP: " + WiFi.localIP().toString() + "\n";
    msg += "⏱ Uptime: " + String(millis() / 1000) + "s\n";
    msg += "🖥 PC: " + String(isPCReachable() ? "online" : "offline / sleeping");
    bot.sendMessage(chatId, msg, "");
    return;
  }
  // unknown command — ignore
}
