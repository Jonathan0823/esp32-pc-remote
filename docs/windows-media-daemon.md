# Windows HTTPS media daemon

The separate Rust tray app lives in [`daemon/`](../daemon/). It targets Windows 11 x64 and preserves the ESP32's [`/v1/media` contract](media-control-https.md); `/v1/actions` accepts only locally configured action IDs. Network requests never provide executable paths or arguments.

## Build and install

The Windows CI job uses Rust 1.92.0/MSVC x64, runs formatting, strict Clippy, tests and a release build, then packages a per-user NSIS installer. The installer does not enable the listener, create firewall rules, elevate the app, or delete `%LOCALAPPDATA%\Esp32MediaDaemon` data on uninstall. Startup is an opt-in per-user setting. Close the tray app before upgrading or uninstalling; the installer does not forcibly terminate it.

For crash diagnosis without building locally, download both artifacts from the **same Windows daemon CI run**:

- `esp32-media-daemon-windows-x64`: the installer.
- `esp32-media-daemon-windows-x64-symbols`: the matching release EXE and PDB debug symbols.

CI keeps release optimizations but enables full debug information and disables symbol stripping. Extract the symbols artifact with the EXE and PDB together. Quit the installed daemon and Settings first (otherwise the single-instance guard exits the new process), then launch the extracted EXE in WinDbg and reproduce the command. When it breaks on an access violation, run `.ecxr` followed by `kb` and share the stack text. Keep the EXE/PDB pair for that run; symbols from another build cannot reliably explain an old fault offset. Do not share raw memory dumps publicly: they may contain API tokens and TLS private keys. This adds diagnostic artifacts, not a crash fix.

For a local Windows build:

```powershell
cd daemon
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --release --locked --target x86_64-pc-windows-msvc
choco install nsis --yes --no-progress
makensis /DAPP_VERSION=0.1.0 installer.nsi
```

The installer appears at `daemon/target/release/esp32-media-daemon-setup.exe`. Unsigned builds may trigger SmartScreen reputation warnings; do not disable Windows security features to work around them.

## Localhost-first HTTPS check

1. Install and launch the daemon. Its listener defaults to **paused** and `127.0.0.1:8443`; no inbound firewall rule is needed for loopback.
2. Open **Settings** from the tray, enable the listener, then use **Export trusted CA PEM**. The PEM is written to `%LOCALAPPDATA%\Esp32MediaDaemon\trusted-ca.pem` only after that explicit action.
3. Use **Show pairing token** and copy it to a local API client. Treat the token as a password; it can trigger every enabled configured action.
4. To use RESTerm, copy `daemon/resterm/resterm.env.example.json` to `resterm.env.json` in that directory, replace `REPLACE_WITH_PAIRING_TOKEN` with the token from Settings, and copy the exported CA there as `trusted-ca.pem`. The local `resterm.env.json` is git-ignored, but stores the token as plaintext—treat it like a password and do not share or commit it. From that directory, run:

   ```powershell
   resterm run --env local --request media .\daemon.http
   ```

   Keep TLS verification enabled. Edit `cmd` directly in the `media` request body (`play`, `pause`, `next`, `previous`, `mute`, or `unmute`). For `/v1/actions`, edit `action_id` directly in the `custom-action` request body to match a locally configured action ID. The scripts fill only the fresh 128-bit request ID and five-second timestamp window. Run `media` for a safe first check, then `custom-action` only if you have configured a suitable action.

   Run `resterm run --env local --request invalid-token .\daemon.http` to expect `401`, or use `--request expired` to expect `410`. For `duplicate-id`, send it twice and expect `200` then `409`; change the sample ID in `scripts/duplicate-request.js` before testing again if that ID has already been used. A `200 executed` / `200 dispatched` response confirms Windows reported that the operation was issued; it does not prove audible playback. Pause the listener in Settings to check that the port stops accepting connections. Rotate credentials only as an explicit user action; this invalidates the old token and CA. Re-export the CA and replace the copy in the RESTerm directory.

   The standalone helper is also available when you want a single smoke test without an API client:

   ```powershell
   python daemon/test_https_daemon.py --ca "$env:LOCALAPPDATA\Esp32MediaDaemon\trusted-ca.pem" --token '<pairing-token>' --host localhost --command pause
   ```

   For a configured action, pass `--action-id <id>` instead of `--command pause`.

Do not disable TLS verification. The server certificate includes `localhost`, `127.0.0.1`, and the selected bind IP. A localhost request validates the daemon/API on that machine only; it does **not** prove ESP32 TLS compatibility, LAN reachability, firewall behavior, or hardware media behavior.

## Pair an ESP32 over the LAN

`127.0.0.1` is reachable only from the PC itself, not from the ESP32. You do not need `0.0.0.0`: the daemon deliberately rejects wildcard binds and supports a specific private LAN IPv4 instead. The ESP32 does not need its own client certificate; it needs the public CA that verifies the daemon, plus the shared pairing token that authorizes requests.

1. Reserve the PC's LAN IPv4 in your router's DHCP settings (for example, `192.168.1.50`). Find the address on the adapter connected to the ESP32's LAN, not a loopback, VPN, or virtual-adapter address.
2. In **Overview**, pause the listener. In **Connection & security**, set **Bind IP** to that LAN IPv4 and keep port `8443` (or choose another port), then **Apply address**. Changing the IP generates a new server certificate with that IP in its SAN and a new CA. Do this **before** exporting the CA; the token is unchanged by an address change.
3. Use **Export trusted CA PEM**, then **Show pairing token**. The CA is written to `%LOCALAPPDATA%\Esp32MediaDaemon\trusted-ca.pem`; the token is the daemon's existing 64-character lowercase hexadecimal API key. Do not generate a different token for the ESP. Keep the server/CA private keys on the PC; never copy them to firmware.
4. In the ESP project's ignored `config.h`, replace the placeholders below. Copy the **entire exported public PEM**, including its BEGIN/END lines, into the raw C++ string, and copy the pairing token exactly:

   ```cpp
   #define PC_MEDIA_HOST "192.168.1.50"
   #define PC_MEDIA_PORT 8443
   #define PC_MEDIA_CA_CERT R"PEM(-----BEGIN CERTIFICATE-----
   <contents of the exported CA certificate>
   -----END CERTIFICATE-----
   )PEM"
   #define PC_MEDIA_API_TOKEN "<daemon pairing token: 64 lowercase hex characters>"
   ```

   `PC_MEDIA_HOST` must match the selected LAN IP, not `localhost`, `127.0.0.1`, or `0.0.0.0`. Keep tokens out of Git, screenshots, logs, and shared build artifacts; the token is embedded in the firmware image. The firmware sends it as `Authorization: Bearer ...` over verified HTTPS. Do not use `setInsecure()` or disable hostname checking.
5. Rebuild and flash the ESP firmware with those settings using the existing USB workflow, or the [OTA workflow](ota.md) if already provisioned. Editing `config.h` alone does not change an installed image. Keep Windows time correct and let the ESP synchronize time over Wi-Fi; it rejects HTTPS commands until time is ready for certificate/expiry validation.
6. For a trusted home LAN, allow inbound TCP on the chosen daemon port through **Windows Defender Firewall**, scoped to the **Private** profile and preferably the reserved ESP32 IP. Resume the listener in **Overview**. Do not disable the firewall, enable a Public-profile rule, or forward this port from the Internet. Client/AP isolation on the router must not block PC-to-ESP traffic.
7. Say “Hi ESP”, then a media phrase. A correlated `200 executed`/white pulse means Windows reported issuing the action, not necessarily audible playback. Check daemon activity and ESP serial categories without logging the token.

If pairing fails: connection refused/timeout points to listener address, firewall, or LAN routing; TLS failures point to an outdated/wrong CA, IP SAN mismatch, or clock; `401` means the token does not match. Fix the corresponding layer rather than bypassing TLS. Changing the bind IP requires exporting the new CA and reflashing the ESP with the new host/CA. **Rotate token and TLS trust** invalidates both values; re-export the CA, copy the new token, and reflash every paired client. Until pairing is complete, leave `PC_MEDIA_HOST` blank to keep ESP media control disabled.

## ESP transport diagnostics

For a failed HTTPS `POST`, firmware prints a bounded, local diagnostic before the usual outcome line:

```text
[media] transport failure cmd=play id=<request-id> http=-1 (connection refused) tls=<code> (<mbedTLS detail>)
[media] heap before POST id=<request-id> internal_free=<bytes> internal_largest=<bytes> psram_free=<bytes> psram_largest=<bytes>
[media] outcome unknown cmd=play id=<request-id> status=-1; not retrying
```

`http=-1` is HTTPClient's generic secure-connect failure: its “connection refused” label can also mean TLS failed, not just a refused TCP socket. Use the accompanying TLS code/detail to distinguish certificate parsing/verification, handshake, and network errors. `tls=0 (no TLS error recorded)` is inconclusive, not proof that TLS succeeded. HTTP `401` is different: TLS completed and the daemon rejected authentication. No token, headers, payload, or certificate contents are logged; keep global HTTP debug logging disabled.

`tls=-32512` (`SSL - Memory allocation failed`) means TLS could not allocate memory; it does not establish whether the certificate is valid. The pinned ESP32-S3 SDK defaults to internal-only mbedTLS allocation. Firmware installs a PSRAM-preferred allocator with an internal-RAM fallback before any services start, shared by media, MQTT, Telegram, and Grafana. Check the boot line `[tls] psram-preferred allocator status=0 psram_free=...` for successful hook installation and available PSRAM. If PSRAM is unavailable, the fallback cannot create extra internal RAM. A failure also prints free bytes and the largest contiguous block in each pool, sampled just before the `POST`; these are snapshots, not measurements of peak handshake use. After flashing this change, verify media commands alongside voice and the enabled network services.

Windows curl's `CERT_TRUST_IS_UNTRUSTED_ROOT` is a separate PC-side trust-chain failure. Confirm the exported CA belongs to the current listener (especially after changing the bind IP or rotating credentials); it does not establish the ESP's failure cause. Rebuild/flash these diagnostics, reproduce once, and inspect the TLS detail without disabling verification or retrying an uncertain action.

## Tray and Settings lifecycle

The tray and HTTPS runtime run in the background process; Settings runs in a separate on-demand process using the same installed executable's internal `--settings` mode. The background process never creates the egui/winit event loop or graphics renderer. Left-click opens Settings; right-click retains the Settings, pause/resume, and Quit menu. Closing Settings exits its process; opening it again starts a fresh UI using current daemon state. Repeated tray clicks or launching the executable again activate the existing Settings process instead of creating another UI or listener. Quitting the daemon closes Settings too. The daemon requests a normal Settings close, then terminates only its owned Settings child after a short timeout (for example, if a native file picker is modal). A Windows job object also ends the Settings child if the daemon process exits unexpectedly.

Settings has three sections: **Overview** for listener/startup status and recent activity, **Actions** for configured applications/scripts, and **Connection & security** for the bind address, token, and CA. A shared microphone/signal icon is embedded in the tray, Settings, executable, installer, and uninstaller; its editable source is `daemon/assets/app.svg`.

In **Actions → Add an action**, choose **Application**, **Batch**, or **PowerShell**, then use **Browse…** to select an `.exe`, `.bat`/`.cmd`, or `.ps1` file. PowerShell also has a separate `.exe` picker for its interpreter. You can still enter absolute paths manually. The native Windows picker is modal: choose a file or cancel it before continuing in Settings. Cancel leaves the existing path unchanged. Selecting a file or adding an action never executes it; **Test…** still requires confirmation. This browses executable files, not an installed-app catalog or document associations. Arguments remain an optional JSON string array so quoting and empty arguments stay explicit.

For Windows acceptance, verify Quit and pause/resume before ever opening Settings, then open/close Settings repeatedly, restore it from minimized through the tray, and relaunch the executable both with Settings open and closed. Confirm there is a separate Settings PID while open, only one Settings window under repeated opens, no Settings process after closing, and continued HTTPS/tray operation. Quit the daemon while Settings is open and while a native picker is open; verify Settings ends and independently launched actions remain unaffected. Check every file filter, cancellation with an existing path, paths containing spaces, and confirmed action execution. Verify the icon in the tray, taskbar, executable, installer, and uninstall entry, including at high DPI. For the memory check, use a release build, warm the listener/command path consistently, record background PID/private committed bytes/working set/handle count, then open and close Settings at least ten times. Sample background memory after the same idle interval and compare it to its warmed pre-GUI baseline; check for a plateau, not an exact 3 MB number or a working-set trim. The separate Settings process should release its GUI allocations when it exits.

The background daemon retains the COM multithreaded apartment (MTA) until after its Tokio runtime and media workers stop. Each worker still balances its own WinRT initialization. This prevents last-worker teardown from invalidating the Windows bindings' cached media-session activation factory between commands. Windows CI exercises repeated factory access on short-lived threads without issuing playback commands; completion does not require a player. On the installed release, also repeat play/pause/next/previous and mute/unmute with Settings both closed and open, allowing idle gaps between commands. Confirm the tray, listener, and background PID remain alive; ordinary media errors must not terminate the daemon. The native crash fix requires that Windows verification before being considered confirmed.

## Configuration and execution safety

- The token and TLS private keys are DPAPI-protected for the current Windows user. The trusted CA is public and is exported only through Settings. Changing the bind IP creates new TLS trust material; explicitly re-pair clients.
- Listener and startup preferences, bind address/port, and action mappings are stored under `%LOCALAPPDATA%\Esp32MediaDaemon`. Startup registration is per-user. Only one app instance is allowed per interactive session.
- Actions are configured locally and addressed remotely only by ID. Application launch passes an argument vector directly. `.bat`/`.cmd` actions use `cmd.exe /D /V:OFF /S /C`; due to cmd's expansion rules, batch paths/arguments are restricted to ASCII letters/digits, spaces and `._-/\:,+()[]=`. Quotes, `%`, `!`, shell metacharacters, and controls are rejected before spawn. PowerShell uses the selected absolute `powershell.exe` or `pwsh.exe` with `-NoProfile -NonInteractive -File`; execution policy is not bypassed.
- Local **Test** requires confirmation. Configured scripts run as the logged-in user, are not sandboxes, and are limited to one at a time with a 60-second process-tree timeout. Application launches are not supervised or killed. Script output is discarded; diagnostics retain only bounded request/action IDs and outcome categories.
- Pause closes the HTTPS listener; it does not undo an action already issued. There is no offline queue or automatic retry.

## Settings process and local management channel

The Settings process is started by the background agent with bounded, versioned JSON messages on its private inherited standard-input/output pipes. The pipes are not a network listener and are created for the owned child; Settings cannot load or write daemon credentials/configuration itself. The background process remains authoritative and applies changes through the existing validators. Protocol frames are size-limited and correlated; requests are not automatically retried if a reply is lost. The general state snapshot contains no pairing token or TLS private key. The token is sent only in response to the explicit **Show pairing token** operation; IPC stdout is not used for logs. A standalone `--settings` launch without the private channel fails closed.

Close both the background daemon and its Settings window before updating or uninstalling. User configuration, DPAPI secrets, and custom actions remain under `%LOCALAPPDATA%` after uninstall.

## Validation status

Linux core/HTTPS tests and strict Clippy cover shared protocol behavior but do not exercise the Windows process, pipe, renderer, or installer. Windows CI must compile and run the Windows API/job-object tests and package the installer. A Windows 11 desktop user must still verify native interaction, process lifecycle, and the repeated-cycle memory procedure above; these manual acceptance checks are pending until performed on the installed release. ESP32/LAN interoperability and gaming performance remain explicitly unverified/deferred.
