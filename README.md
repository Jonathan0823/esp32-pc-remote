# esp32-pc-remote

ESP32 Telegram bot for remote PC control.

Wake a home PC, check whether it is online, switch between machines, and reboot the ESP32 from Telegram.

## What it does
- Confirm before sending Wake-on-LAN to the selected PC
- Check whether the PC is online via TCP probe
- Switch between target machines
- Report ESP32 health over Telegram
- Recover the ESP32 from Telegram with `/reboot`
- Send diagnostics and event logs to Grafana Cloud (optional)

## Commands
- `/start` or `/help` — show the menu and current target
- `/ping` — ESP32 diagnostics (including reset reason, poll health)
- `/status` — ESP32 health + target PC status
- `/wake` — ask for confirmation before waking the active target
- `/wakeconfirm <name>` — confirm and send WoL to the active target
- `/devices` — list known machines
- `/target <name>` — switch active PC
- `/reboot` — restart the ESP32

## Requirements
- ESP32 board support in Arduino CLI
- `UniversalTelegramBot` library
- A Telegram bot token
- A PC with WoL enabled
- A TCP port on the PC that the ESP32 can probe for online status

## Setup
1. Copy `config.example.h` to `config.h`
2. Fill in Wi‑Fi, Telegram, and PC values
3. Build and upload:
   ```bash
   arduino-cli compile --fqbn esp32:esp32:esp32 .
   arduino-cli upload --fqbn esp32:esp32:esp32 -p /dev/ttyUSB0 .
   ```

## Configuration
`config.h` is ignored by git. Keep your real Wi‑Fi password and bot token there.

The example config currently supports one PC, but the code is structured so more targets can be added later.

## Grafana Cloud logging (optional)
The ESP can push event logs directly to Grafana Cloud Loki for diagnostics and dashboards.

1. Create a **free Grafana Cloud** account at https://grafana.com
2. Get your **Logs** instance URL, username, and API key
3. Fill in `config.h`:
   ```c
   #define GRAFANA_LOGS_URL   "https://<instance>.grafana.net"
   #define GRAFANA_LOGS_USER  "<logs-instance-id>"
   #define GRAFANA_LOGS_TOKEN "<grafana-cloud-api-token>"
   ```
4. Leave them blank to skip cloud logging entirely.

What gets logged:
- Boot / reset reason
- Wi‑Fi connect / reconnect / disconnect
- Telegram poll failures
- Wake-on-LAN events
- Command usage
- Heartbeat on every flush (30s interval)

Log format:
```json
{"level":"info","component":"boot","event":"start","msg":"ESP32 PC Remote started","uptime_s":0,"heap":48320,"rssi":-56,"ip":"192.168.1.100"}
```

Logs arrive in a single Loki stream labeled `{app="esp32-pc-remote"}`.

## /ping diagnostics
- 📡 RSSI
- 🌐 IP address
- ⏱ Uptime
- 💾 Free heap
- 🔄 Last reset reason (brownout, poweron, panic, etc.)
- 📬 Time since last successful Telegram poll
- ⚠️ Telegram poll failure count

Tip: check `/ping` first when the ESP seems offline — it shows whether the ESP crashed, had a brownout, or lost Wi‑Fi.

## Public release checklist
- Do not commit `config.h`
- Rotate the Telegram bot token if it was ever shared outside your machine
- Keep private network details out of screenshots, logs, and issues
- Review `.gitignore` before adding new local files

## License
MIT
