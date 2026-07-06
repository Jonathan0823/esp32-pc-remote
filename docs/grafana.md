# Grafana Cloud Loki setup

## 1. Create a Grafana Cloud account

Free tier at [grafana.com](https://grafana.com).

## 2. Get your Logs credentials

In your Grafana Cloud portal, go to **Details > Loki** and note:
- **URL** — e.g. `https://<instance>.grafana.net`
- **Username** — your Logs instance ID
- **API key** — create one under **Access Policies**

## 3. Fill in `config.h`

```c
#define GRAFANA_LOGS_URL   "https://<instance>.grafana.net"
#define GRAFANA_LOGS_USER  "<logs-instance-id>"
#define GRAFANA_LOGS_TOKEN "<grafana-cloud-api-token>"
```

Leave them blank to skip cloud logging entirely.

## What gets logged

- Boot / reset reason
- WiFi connect / reconnect / disconnect
- Watchdog-triggered resets
- Heartbeat every 60s while healthy
- Telegram poll failures (after short streak) and recovery
- No command usage or wake noise

## Stream label

All logs arrive at `{app="esp32-pc-remote"}`.

## Suggested alerts

- Heartbeat missing > 3 minutes
- WiFi lost for > 1 minute
- Poll fail streak >= 3
- Reset reason = brownout or panic
