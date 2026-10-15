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
- Wake detections/requests and packet-submission outcomes, tagged with source (`voice`, `mqtt`, or `telegram`) and a per-attempt `wake_id`
- Rejected attempts with reasons such as voice cooldown, disconnected Wi-Fi, or an already-pending MQTT request
- Telegram poll failures (after short streak) and recovery
- No routine command usage or probe noise

## Stream label

All logs arrive at `{app="esp32-pc-remote"}`.

## Investigate unexpected PC wakes

Filter structured wake records by source:

```logql
{app="esp32-pc-remote"} | json | component="wake" | source="voice"
{app="esp32-pc-remote"} | json | component="wake" | source="mqtt"
```

Each request uses a `wake_id`; filter on it to group its records. `uptime_ms` records when the event happened, even if Wi-Fi delays delivery. `request_received` means the ESP32 recognized/received a request, while `request_rejected` gives the reason it did not proceed. `packet_submission` records `sent: true` or `sent: false` and does not prove the PC started. Voice records include detections rejected by cooldown or disconnected Wi-Fi. Queue overflow is reported on the Serial log; the Grafana queue is bounded, and cloud delivery is best-effort. MQTT identifies the transport, not which broker client sent the command. Wake records include the configured PC name/MAC and broadcast address. A PC wake with no corresponding event may have come from another LAN sender, a BIOS/OS wake timer, or another source outside this ESP32.

## Suggested alerts

- Heartbeat missing > 3 minutes
- WiFi lost for > 1 minute
- Poll fail streak >= 3
- Reset reason = brownout or panic
