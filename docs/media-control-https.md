# LAN media-control HTTPS contract

This contract is shared by the ESP32 client and the [Windows HTTPS daemon](windows-media-daemon.md). For a real ESP-to-PC connection, follow the [LAN pairing walkthrough](windows-media-daemon.md#pair-an-esp32-over-the-lan): select a private LAN bind IP, export the daemon's CA and pairing token, and provision both in the ESP firmware. The Python fixture below is only a non-actioning test server.

## Connection and authentication

- The PC hosts HTTPS; the ESP connects to `https://<PC_MEDIA_HOST>:<PC_MEDIA_PORT>/v1/media` (default port `8443`). Reserve the PC address in DHCP. The configured host must match the certificate SAN.
- The ESP verifies certificate chain and host using `PC_MEDIA_CA_CERT`; no plain HTTP, insecure fallback, redirects, or credential forwarding to another host.
- `Authorization: Bearer <PC_MEDIA_API_TOKEN>` carries a per-pair, randomly generated 32-byte token encoded as 64 lowercase hex characters. Compare tokens in constant time. Do not reuse HiveMQ credentials.
- Provision `PC_MEDIA_HOST`, `PC_MEDIA_PORT`, `PC_MEDIA_CA_CERT`, and `PC_MEDIA_API_TOKEN` locally in ignored `config.h`; `config.example.h` defaults leave media disabled. For the Windows daemon, copy its existing pairing token from Settings and use its exported public CA PEM (as a raw C++ string or with escaped `\n` line breaks); do not generate a different ESP token. Only for the disposable Python fixture, generate a token with `openssl rand -hex 32`. Never commit, log, or place the token in the URL/body. Rotate by reprovisioning both devices. No WAN port forwarding.

## Request

One HTTPS `POST` with `Content-Type: application/json` and a body no larger than 512 bytes:

The `/v1` path selects the API version; the body carries no separate version field.

```json
{"id":"0123456789abcdef0123456789abcdef","cmd":"pause","issued_at_ms":1783586658000,"expires_at_ms":1783586663000}
```

- `id`: exactly 32 lowercase hexadecimal characters from 128 random bits; unique per request.
- `cmd`: exactly one of `play`, `pause`, `next`, `previous`, `mute`, `unmute`.
- `issued_at_ms`, `expires_at_ms`: unsigned Unix milliseconds; expiry must equal issue time plus 5000 ms.
- Reject unknown or missing fields, wrong types, oversized input, unsupported commands, and invalid IDs.
- Expiry begins when the ESP recognizes the command. The receiver allows at most 5000 ms future issue-time skew and rejects at `now >= expires_at_ms`. Excessive future skew returns `400 clock_skew`; an expired request returns `410 expired`. Do not grant expiry grace. The ESP also checks monotonic age before enqueue and send; expired requests are discarded, never refreshed or retried.

## Responses and duplicates

- `202 Accepted` with `{"id":"<same-id>","status":"accepted"}` means validated/accepted, **not executed**. The test server only returns this state.
- `200 OK` with `{"id":"<same-id>","status":"executed"}` means the Windows daemon issued the native media action; it does not prove playback state.
- The URL path selects the API version; correlate responses by ID. Never display an execution-success indication for `202`, a malformed response, or a mismatched ID.
- Receiver keeps at most 128 request IDs through their five-second acceptance windows; duplicate is `409`, cache full is `429`. Reject saturation instead of evicting unexpired IDs. No persistent exactly-once guarantee across receiver crashes.
- Other receiver statuses: `400` malformed/time or `clock_skew`, `401` unauthorized, `404`/`405` wrong path/method, `410` expired, `413` oversized, `5xx` server failure. Response bodies are capped at 512 bytes. Unknown outcome after the request may have been written is not retried.
- The receiver authenticates and validates the full request before dispatch. `/v1/media` allows only the six built-in media actions; the daemon's separate `/v1/actions` endpoint accepts only IDs for locally configured actions, never executable paths or arguments.

## Voice configuration and custom actions

The paired ESP32 fetches configuration only after the built-in **Jarvis → update** command. Authenticated `GET /v1/voice-config` returns the latest published snapshot (`schema`, `revision`, `actions_revision`, and `commands` containing `phrase`/`action_id`). The device rejects stale action revisions, invalid schemas, malformed or oversized bodies (3,072 bytes maximum), unsupported phrases, duplicate phrases/action IDs, and built-in words. It supports at most 12 custom phrases, each 2–63 ASCII letters and single spaces. The phrase table is persisted in alternating NVS slots; activation changes the active slot only after the recognizer accepts the complete table.

After successful activation, the ESP sends `POST /v1/voice-config/ack` with `{"revision":<revision>}`. Only a matching current revision is acknowledged as active in Settings. A failed fetch, validation, recognizer rebuild, or persistence step leaves the previous published/active status unconfirmed; the old vocabulary is restored when possible. Sync occurs outside the speech callback and is never retried automatically. A successful vocabulary replacement briefly restarts recognition; do not sync during a command window (the firmware defers it until that session ends).

Custom speech actions use the existing authenticated `/v1/actions` endpoint with an action ID and published voice revision. The daemon rejects deleted/disabled actions or stale revisions before dispatch; the ESP never receives executable paths or script arguments. Action requests retain the existing five-second lifetime and are never retried.

## ESP behavior and feedback

- “Jarvis” only opens the existing approximately 5.76-second listening window. One valid phrase consumes it. “Wake computer” continues to send local WoL and never calls this endpoint.
- The voice callback/worker never performs TLS or waits for the PC. A bounded handoff feeds one dedicated HTTPS worker; there is at most one active and one waiting media request. A full queue, disconnected Wi-Fi, missing configuration, untrusted certificate, unsynchronized clock, or stale request is rejected immediately; no offline queue or automatic retry.
- Configure time asynchronously after Wi-Fi starts, independently of optional Grafana. Fail closed until system time is at least 2025-01-01 UTC and TLS certificate-time validation is possible.
- Use bounded HTTP operations and parse at most 512 response bytes. Disable redirects. Keep diagnostics redacted and local; no token in Serial, MQTT, Grafana, or exceptions.
- LED uses the existing single owner: blue while listening; yellow on queued/accepted; white only for a correlated `200 executed` media reply or `200 dispatched` custom-action reply; red for rejection, failure, or unknown outcome. Pulses are 500 ms, dim (no brighter than the existing indicator setting), and restore blue if still listening. Red means execution was not confirmed, not that the action definitely did not occur. Serial gives the precise result.

## Test server

`scripts/media_https_test_server.py` is a loopback-default Python standard-library fixture. It validates the request/token/expiry/duplicates and responds `202 accepted`; it never runs a PC action or responds `executed`. Host tests generate disposable localhost certificates with OpenSSL and exercise verified TLS, duplicate rejection, authentication, and hostname mismatch.

For a manual ESP-on-LAN test, use a disposable token and a certificate with a SAN matching the PC's reserved LAN IPv4 address (replace the example address):

```sh
mkdir -p /tmp/media-test
openssl rand -hex 32 > /tmp/media-test/token
openssl req -x509 -newkey rsa:2048 -nodes -days 2 \
  -keyout /tmp/media-test/server-key.pem -out /tmp/media-test/ca.pem \
  -subj '/CN=ESP media test' \
  -addext 'subjectAltName=IP:192.168.1.50' \
  -addext 'basicConstraints=critical,CA:TRUE' \
  -addext 'keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign' \
  -addext 'extendedKeyUsage=serverAuth'
```

Set `PC_MEDIA_HOST` to that IP, `PC_MEDIA_CA_CERT` to the PEM certificate as a C string with `\n` line breaks, and `PC_MEDIA_API_TOKEN` to the generated token in ignored `config.h`; flash only with permission. Load the same disposable token into the fixture environment without printing it. Start the fixture with `--host 0.0.0.0 --port 8443 --cert /tmp/media-test/ca.pem --key /tmp/media-test/server-key.pem` only for the LAN test, and restrict the host firewall to the private LAN. The default `127.0.0.1` bind is safer for host-only tests. Clean up temporary keys/tokens after testing. Never use production credentials in tests.

For real Windows actions, use the [Windows daemon and LAN pairing guide](windows-media-daemon.md#pair-an-esp32-over-the-lan), not this fixture. Windows/ESP interoperability and game performance still require validation on the target devices.
