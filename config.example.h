#pragma once

// Copy this file to config.h and fill in your values.
// config.h is gitignored; config.example.h is the template.

#define WIFI_SSID     "your-wifi-ssid"
#define WIFI_PASS     "your-wifi-password"
#define BOT_TOKEN     "1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"

// Wake-on-LAN
#define PC_NAME     "your-pc-hostname"          // optional, for display
#define PC_MAC      "aa:bb:cc:dd:ee:ff"       // target PC MAC (required)
#define PC_IP       "192.168.1.50"              // target PC IP (for /status check)
#define WOL_BCAST   "192.168.1.255"             // LAN broadcast address
#define WOL_PORT    9                            // WoL port (usually 7 or 9)
#define PC_TCP_PORT  47989                         // TCP port for /status probe

// Grafana Cloud Loki (optional — leave blank to skip cloud logging)
#define GRAFANA_LOGS_URL   ""                                 // e.g. "https://<instance>.grafana.net"
#define GRAFANA_LOGS_USER  ""                                 // Logs instance ID (username)
#define GRAFANA_LOGS_TOKEN ""                                 // Grafana Cloud API token (password)
