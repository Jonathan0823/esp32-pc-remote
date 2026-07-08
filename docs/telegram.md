# Telegram bot setup

## 1. Create a bot

1. Open Telegram and search for [@BotFather](https://t.me/BotFather)
2. Send `/newbot` and follow the prompts
3. Save the **bot token** — you'll need it for `config.h`

## 2. Fill in `config.h`

```c
#define BOT_TOKEN "1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"
```

## 3. Find your bot

Search for the username you chose (e.g. `@MyPcRemoteBot`) and start a chat.

## Commands reference

| Command | Description |
|---------|-------------|
| `/help` or `/start` | Show the menu |
| `/ping` | ESP32 diagnostics — reset reason, heap, WiFi RSSI, poll health |
| `/status` | ESP32 health + target PC online state |
| `/wake` | Request wake confirmation for the target PC (inline buttons) |
| `/reboot` | Reboot the ESP32 |

The bot only responds to the commands above. Anything else gets a "Unknown command" hint.

## Troubleshooting

**Bot doesn't reply**
- Check `BOT_TOKEN` in `config.h` — it must match the token from BotFather
- Check WiFi — does `/ping` show a connected state?
- If the ESP32 just rebooted, wait a few seconds for WiFi + Telegram poll

**"Unknown command" for a valid command**
- Make sure there's no extra space before the `/`
- Commands are case-insensitive but must match the prefix exactly
