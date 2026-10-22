# MQTT control

Use MQTT to send wake commands and view device/PC status, or connect the [private web dashboard](../dashboard/README.md). MQTT and the dashboard do not control Windows media or launch configured applications/scripts; those actions are voice-triggered through the paired Windows companion. Start with the [firmware quickstart](../README.md#quickstart-wake-your-pc-by-voice), then configure the broker and rebuild/upload the firmware.

MQTT is optional. Configure these values in `config.h` to enable MQTT over TLS, normally on port `8883`:

- `MQTT_BROKER`
- `MQTT_PORT`
- `MQTT_USER`
- `MQTT_PASS`
- `MQTT_BASE_TOPIC` (for example, `home/pc-remote/desktop-01`)

The PlatformIO configuration installs the MQTT library automatically; leave the broker settings blank to disable MQTT.

Topics under `MQTT_BASE_TOPIC`:

| Topic           | Retain | Purpose                    |
| --------------- | ------ | -------------------------- |
| `/availability` | Yes    | ESP32 online/offline state |
| `/state`        | Yes    | Latest ESP32 and PC status |
| `/cmd`          | No     | Wake and ESP32 reboot commands |
| `/reply`        | No     | Command replies            |
| `/event`        | No     | One-time events            |
| `/log`          | No     | Live debug logs            |

Commands on `/cmd`:

```text
ping
wake_request
wake_confirm
reboot_request
reboot_confirm
```

`wake_request` accepts `force: true` to skip confirmation. `/state` is refreshed when values change and every 60 seconds, and includes `pc_online` and `pc_status`.

Example wake request:

```json
{
  "id": "wake-001",
  "cmd": "wake_request",
  "target": "desktop-pc",
  "force": true,
  "expires_in_s": 30,
  "ts": 1783586658
}
```

If Telegram and MQTT are both configured, both control paths remain available.


## Security

Use dedicated MQTT users with restricted topic ACLs; never retain command messages. The ESP32's current MQTT client uses TLS but disables server-certificate verification. Encryption alone does not establish broker identity. Dashboard credentials are embedded in its browser bundle; protect the deployment with access control and restrict its MQTT user as described in the [dashboard guide](../dashboard/README.md#risk).
