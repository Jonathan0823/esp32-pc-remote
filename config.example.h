#pragma once

// Copy this file to config.h and fill in your values.
// config.h is gitignored; config.example.h is the template.

#define WIFI_SSID     "your-wifi-ssid"
#define WIFI_PASS     "your-wifi-password"
#define BOT_TOKEN     ""                                     // optional; leave blank to disable Telegram

// Wake-on-LAN
#define PC_NAME     "your-pc-hostname"          // optional, for display
#define PC_MAC      "aa:bb:cc:dd:ee:ff"       // target PC MAC (required)
#define PC_IP       "192.168.1.50"              // target PC IP (for /status check)
#define WOL_BCAST   "192.168.1.255"             // LAN broadcast address
#define WOL_PORT    9                            // WoL port (usually 7 or 9)
#define PC_TCP_PORT  47989                         // TCP port for /status probe

// MQTT (optional — leave blank to skip MQTT)
#define MQTT_BROKER     ""                                     // e.g. "mqtt.example.com"
#define MQTT_PORT       8883                                    // TLS port
#define MQTT_USER       ""                                     // MQTT username
#define MQTT_PASS       ""                                     // MQTT password
#define MQTT_BASE_TOPIC ""                                     // e.g. "ejo-pc-remote-8f3k29/desktop-01"

// Optional LAN media control; pair with the Windows daemon (docs/windows-media-daemon.md)
#define PC_MEDIA_HOST ""                                       // daemon's reserved LAN IPv4 (not 127.0.0.1); blank disables media control
#define PC_MEDIA_PORT 8443
#define PC_MEDIA_CA_CERT ""                                    // daemon's exported trusted-ca.pem as a C string; never a private key
#define PC_MEDIA_API_TOKEN ""                                  // same 64 lowercase hex pairing token shown by daemon Settings
#define PC_OTA_PASSWORD ""                                    // optional; 64 lowercase hex characters; leave blank to disable OTA

// Grafana Cloud Loki (optional — leave blank to skip cloud logging)
#define GRAFANA_LOGS_URL   ""                                 // e.g. "https://<instance>.grafana.net"
#define GRAFANA_LOGS_USER  ""                                 // Logs instance ID (username)
#define GRAFANA_LOGS_TOKEN ""                                 // Grafana Cloud API token (password)
