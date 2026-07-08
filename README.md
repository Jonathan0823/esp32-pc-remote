# esp32-pc-remote

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![ESP32](https://img.shields.io/badge/Board-ESP32-green)](https://www.espressif.com/)
[![Telegram Bot](https://img.shields.io/badge/Telegram-Bot-0088cc)](https://core.telegram.org/bots)

Wake your PC from anywhere — no static IP, no port forwarding, no cloud subscription.

Just an ESP32, a Telegram bot, and a WiFi connection. Send `/wake`, the ESP32 sends a Wake-on-LAN magic packet on your LAN.

## Quickstart

```bash
cp config.example.h config.h   # fill in WiFi + bot token + PC MAC
arduino-cli compile --fqbn esp32:esp32:esp32 .
arduino-cli upload --fqbn esp32:esp32:esp32 -p /dev/ttyUSB0 .
```

Send `/help` to your bot. That's it.

## What it does

- **Wake your PC** from Telegram with confirmation (`/wake` → inline buttons)
- **Check if it's online** via TCP probe (`/status`)
- **Diagnose the ESP32** — reset reason, heap, WiFi RSSI, poll health (`/ping`)
- **Auto-recover** — watchdog reboots on hang, WiFi reconnects with backoff
- **Grafana Cloud logging** (optional) — uptime + alert-only events

## Commands

| Command | What it does |
|---------|-------------|
| `/help` or `/start` | Show menu |
| `/ping` | ESP32 health: reset reason, heap, RSSI, poll stats |
| `/status` | ESP32 health + target PC online status |
| `/wake` | Ask for wake confirmation |
| `/reboot` | Reboot the ESP32 |

See [Telegram bot setup →](docs/telegram.md) for creating the bot and getting your token.

## Requirements

- Any ESP32 board
- [Arduino CLI](https://arduino.github.io/arduino-cli/) with `esp32:esp32` core
- `UniversalTelegramBot` library
- A PC with [Wake-on-LAN configured](docs/wake-on-lan.md) (BIOS + OS + NIC)
- A TCP port on the PC for online probing (e.g. 47989 for Moonlight)

## Troubleshooting

**PC won't wake**
- Verify WoL is enabled in BIOS and OS (check your NIC driver settings)
- Send WoL from another tool first — if it works there, the ESP32 config is wrong

**ESP32 keeps rebooting**
- Check `/ping` for the reset reason
- `task_wdt` = main loop hung (watchdog saved you)
- `brownout` = power supply too weak

**Bot doesn't respond**
- Check WiFi: does `/ping` show an IP?
- Check the bot token in `config.h`

## Grafana Cloud logging (optional)

Push boot, WiFi, and watchdog events to Grafana Cloud Loki.

1. Create a free [Grafana Cloud](https://grafana.com) account
2. Fill `GRAFANA_LOGS_URL`, `GRAFANA_LOGS_USER`, `GRAFANA_LOGS_TOKEN` in `config.h`
3. Leave blank to skip

Logs go to stream `{app="esp32-pc-remote"}`. Only critical state changes are sent — no command noise.  
[Detailed setup →](docs/grafana.md)

## Contributing

Issues and PRs welcome. Keep `config.h` out of commits. See [CONTRIBUTING](CONTRIBUTING.md).

## License

MIT

---

If this is useful, [star the repo](https://github.com/Jonathan0823/esp32-pc-remote) ⭐
