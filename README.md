# esp32-pc-remote

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![ESP32](https://img.shields.io/badge/Board-ESP32-green)](https://www.espressif.com/)
[![Telegram Bot](https://img.shields.io/badge/Telegram-Bot-0088cc)](https://core.telegram.org/bots)

Wake your PC from anywhere — no static IP, no port forwarding, no cloud subscription.

Just an ESP32, a Telegram bot, and/or an MQTT broker. Send `/wake` or publish an MQTT command, and the ESP32 sends a Wake-on-LAN magic packet on your LAN.

## Quickstart

```bash
cp config.example.h config.h   # fill in WiFi + bot token and/or MQTT settings + PC MAC
arduino-cli compile --fqbn esp32:esp32:esp32 .
arduino-cli upload --fqbn esp32:esp32:esp32 -p /dev/ttyUSB0 .
```

If using Telegram, send `/help` to your bot. That's it.

## What it does

- **Wake your PC** from Telegram with confirmation or force (`/wake` → inline buttons, `/wake force`)
- **Check if it's online** via TCP probe (`/status`)
- **Diagnose the ESP32** — reset reason, heap, WiFi RSSI, poll health (`/ping`)
- **Auto-recover** — watchdog reboots on hang, WiFi reconnects with backoff
- **Control via MQTT** (optional) — TLS broker support with retained state, replies, and events
- **Grafana Cloud logging** (optional) — uptime + alert-only events

## Commands

| Command | What it does |
|---------|-------------|
| `/help` or `/start` | Show menu |
| `/ping` | ESP32 health: reset reason, heap, RSSI, poll stats |
| `/status` | ESP32 health + target PC online status |
| `/wake` | Ask for wake confirmation |
| `/wake force` | Wake immediately without confirmation |
| `/reboot` | Reboot the ESP32 |

See [Telegram bot setup →](docs/telegram.md) for creating the bot and getting your token.
Leave `BOT_TOKEN` blank to disable Telegram and use MQTT only.

## MQTT (optional)

Fill these in `config.h` to enable MQTT over TLS (port 8883):

- `MQTT_BROKER`
- `MQTT_PORT`
- `MQTT_USER`
- `MQTT_PASS`
- `MQTT_BASE_TOPIC` (e.g. `ejo-pc-remote-8f3k29/desktop-01`)

Topics under `MQTT_BASE_TOPIC`:

| Topic | Retain | Purpose |
|-------|--------|---------|
| `/availability` | Yes | ESP32 online/offline |
| `/state` | Yes | Latest status + PC online/offline |
| `/cmd` | No | Action commands |
| `/reply` | No | Command replies |
| `/event` | No | One-time events |
| `/log` | No | Live debug logs |

Commands on `/cmd`: `ping`, `wake_request`, `wake_confirm`, `reboot_request`, `reboot_confirm`.

`wake_request` accepts `force: true` to skip confirmation.

`/state` is refreshed on changes and every 60s, and includes `pc_online` / `pc_status`.

Example `wake_request` payload:

```json
{"id":"wake-001","cmd":"wake_request","target":"desktop-pc","force":true,"expires_in_s":30,"ts":1783586658}
```

Leave `BOT_TOKEN` blank to disable Telegram.

Telegram `/wake force` skips confirmation too.

If both Telegram and MQTT are configured, MQTT is primary and Telegram still works.

## Requirements

- Any ESP32 board
- [Arduino CLI](https://arduino.github.io/arduino-cli/) with `esp32:esp32` core
- `UniversalTelegramBot` library (only needed if Telegram is enabled)
- `PubSubClient` library (only needed if MQTT is enabled)
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
- If using Telegram, check the bot token in `config.h`

**MQTT doesn't connect**
- Check `MQTT_BROKER`, `MQTT_PORT`, `MQTT_USER`, `MQTT_PASS`, and `MQTT_BASE_TOPIC`
- Confirm the broker accepts TLS on port 8883

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
