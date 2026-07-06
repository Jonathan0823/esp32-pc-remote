# esp32-pc-remote

ESP32 Telegram bot for remote PC control.

Wake a home PC, check whether it is online, switch between machines, and reboot the ESP32 from Telegram.

## What it does
- Confirm before sending Wake-on-LAN to the selected PC
- Check whether the PC is online
- Switch between target machines
- Report ESP32 health over Telegram
- Recover the ESP32 from Telegram with `/reboot`

## Commands
- `/start` or `/help` — show the menu and current target
- `/ping` — ESP32 diagnostics
- `/status` — ESP32 health + target status
- `/wake` — ask for confirmation before waking the active target
- `/wakeconfirm <name>` — confirm and send WoL to the active target
- `/devices` — list known machines
- `/target <name>` — switch active target
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

## Public release checklist
- Do not commit `config.h`
- Rotate the Telegram bot token if it was ever shared outside your machine
- Keep private network details out of screenshots, logs, and issues
- Review `.gitignore` before adding new local files

## License
MIT
