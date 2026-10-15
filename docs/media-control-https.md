# LAN media-control HTTPS contract

This contract is for the ESP32 client and the future Windows agent. Task 002 implements the ESP client and a non-actioning test server only; it does not execute Windows media actions.

## Connection and authentication

- The PC hosts HTTPS; the ESP connects to `https://<PC_MEDIA_HOST>:<PC_MEDIA_PORT>/v1/media` (default port `8443`). Reserve the PC address in DHCP. The configured host must match the certificate SAN.
- The ESP verifies certificate chain and host using `PC_MEDIA_CA_CERT`; no plain HTTP, insecure fallback, redirects, or credential forwarding to another host.
- `Authorization: Bearer <PC_MEDIA_API_TOKEN>` carries a per-pair, randomly generated 32-byte token encoded as 64 lowercase hex characters. Compare tokens in constant time. Do not reuse HiveMQ credentials.
- Provision `PC_MEDIA_HOST`, `PC_MEDIA_PORT`, `PC_MEDIA_CA_CERT`, and `PC_MEDIA_API_TOKEN` locally in ignored `config.h`; `config.example.h` defaults leave media disabled. Generate a disposable token with `openssl rand -hex 32`; put the trusted CA PEM in the C string with escaped `\n` line breaks. Never commit, log, or place the token in the URL/body. Rotate by reprovisioning both devices. No WAN port forwarding.

## Request

One HTTPS `POST` with `Content-Type: application/json` and a body no larger than 512 bytes:

```json
{"v":1,"id":"0123456789abcdef0123456789abcdef","cmd":"pause","issued_at_ms":1783586658000,"expires_at_ms":1783586663000}
```

- `v`: integer `1`.
- `id`: exactly 32 lowercase hexadecimal characters from 128 random bits; unique per request.
- `cmd`: exactly one of `play`, `pause`, `next`, `previous`, `mute`, `unmute`.
- `issued_at_ms`, `expires_at_ms`: unsigned Unix milliseconds; expiry must equal issue time plus 5000 ms.
- Reject unknown or missing fields, wrong types, oversized input, unsupported commands/versions, and invalid IDs.
- Expiry begins when the ESP recognizes the command. The receiver allows at most 1000 ms future issue-time skew and rejects at `now >= expires_at_ms`. Do not grant expiry grace. The ESP also checks monotonic age before enqueue and send; expired requests are discarded, never refreshed or retried.

## Responses and duplicates

- `202 Accepted` with `{"v":1,"id":"<same-id>","status":"accepted"}` means validated/accepted, **not executed**. The test server only returns this state.
- `200 OK` with the same ID and `{"status":"executed"}` means the future Windows agent issued the native media action; it does not prove playback state.
- Correlate both version and ID. Never display an execution-success indication for `202`, a malformed response, or a mismatched ID.
- Receiver keeps at most 128 request IDs through their five-second acceptance windows; duplicate is `409`, cache full is `429`. Reject saturation instead of evicting unexpired IDs. No persistent exactly-once guarantee across receiver crashes.
- Other receiver statuses: `400` malformed/version/time, `401` unauthorized, `404`/`405` wrong path/method, `410` expired, `413` oversized, `5xx` server failure. Response bodies are capped at 512 bytes. Unknown outcome after the request may have been written is not retried.
- The future receiver authenticates and validates the full request before dispatch. It allows only the six built-in media actions; no shell or arbitrary process execution.

## ESP behavior and feedback

- “Hi ESP” only opens the existing approximately 5.76-second listening window. One valid phrase consumes it. “Wake computer” continues to send local WoL and never calls this endpoint.
- The voice callback/worker never performs TLS or waits for the PC. A bounded handoff feeds one dedicated HTTPS worker; there is at most one active and one waiting media request. A full queue, disconnected Wi-Fi, missing configuration, untrusted certificate, unsynchronized clock, or stale request is rejected immediately; no offline queue or automatic retry.
- Configure time asynchronously after Wi-Fi starts, independently of optional Grafana. Fail closed until system time is at least 2025-01-01 UTC and TLS certificate-time validation is possible.
- Use bounded HTTP operations and parse at most 512 response bytes. Disable redirects. Keep diagnostics redacted and local; no token in Serial, MQTT, Grafana, or exceptions.
- LED uses the existing single owner: blue while listening; yellow on queued/accepted; white only for a correlated `200 executed`; red for rejection, failure, or unknown outcome. Pulses are 500 ms, dim (no brighter than the existing indicator setting), and restore blue if still listening. Red means execution was not confirmed, not that the action definitely did not occur. Serial gives the precise result.

## Test server

`scripts/media_https_test_server.py` is a loopback-only Python standard-library fixture. It validates the request/token/expiry/duplicates and responds `202 accepted`; it never runs a PC action or responds `executed`. Host tests generate disposable localhost certificates with OpenSSL and exercise verified TLS, duplicate rejection, authentication, and hostname mismatch.

For a manual ESP-on-LAN test, use a disposable token and a certificate with a SAN matching the PC's reserved LAN IPv4 address (replace the example address):

```sh
mkdir -p /tmp/media-test
openssl rand -hex 32 > /tmp/media-test/token
openssl req -x509 -newkey rsa:2048 -nodes -days 2 \
  -keyout /tmp/media-test/server-key.pem -out /tmp/media-test/ca.pem \\
  -subj '/CN=ESP media test' \\
  -addext 'subjectAltName=IP:192.168.1.50' \\
  -addext 'basicConstraints=critical,CA:TRUE' \\
  -addext 'keyUsage=critical,digitalSignature,keyEncipherment,keyCertSign' \\
  -addext 'extendedKeyUsage=serverAuth'
```

Set `PC_MEDIA_HOST` to that IP, `PC_MEDIA_CA_CERT` to the PEM certificate as a C string with `\n` line breaks, and `PC_MEDIA_API_TOKEN` to the generated token in ignored `config.h`; flash only with permission. Load the same disposable token into the fixture environment without printing it. Start the fixture with `--host 0.0.0.0 --port 8443 --cert /tmp/media-test/ca.pem --key /tmp/media-test/server-key.pem` only for the LAN test, and restrict the host firewall to the private LAN. The default `127.0.0.1` bind is safer for host-only tests. Clean up temporary keys/tokens after testing. Never use production credentials in tests.

The Windows HTTPS daemon, Windows-protected token storage, GUI/firewall setup, native actions, and game performance measurements are deferred to a separate task.
