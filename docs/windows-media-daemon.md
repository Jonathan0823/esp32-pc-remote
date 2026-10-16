# Windows HTTPS media daemon

The separate Rust tray app lives in [`daemon/`](../daemon/). It targets Windows 11 x64 and preserves the ESP32's [`/v1/media` contract](media-control-https.md); `/v1/actions` accepts only locally configured action IDs. Network requests never provide executable paths or arguments.

## Build and install

The Windows CI job uses Rust 1.92.0/MSVC x64, runs formatting, strict Clippy, tests and a release build, then packages a per-user NSIS installer. The installer does not enable the listener, create firewall rules, elevate the app, or delete `%LOCALAPPDATA%\Esp32MediaDaemon` data on uninstall. Startup is an opt-in per-user setting. Close the tray app before upgrading or uninstalling; the installer does not forcibly terminate it.

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
4. Import `daemon/postman-collection.json` and `daemon/postman-environment.json` into Postman. Set the environment `token`; leave Postman's SSL certificate verification enabled and trust the exported CA. Or run:

   ```powershell
   python daemon/test_https_daemon.py --ca "$env:LOCALAPPDATA\Esp32MediaDaemon\trusted-ca.pem" --token '<pairing-token>' --host localhost --command pause
   ```

   The helper creates a fresh 128-bit request ID and five-second timestamp window for every request. `200 executed` means Windows reported that the operation was issued; it does not prove audible playback. For a locally configured custom action, set `actionId` in Postman or pass `--action-id <id>` to the helper.
5. Check invalid-token (`401`), expired (`410`), duplicate (`409`), unknown/disabled action (`404`/`403`), and paused-listener behavior. Send the duplicate collection request twice; the second request reuses the same ID. Rotate credentials only as an explicit user action; this invalidates the old token and CA. Re-export the new CA and pair again.

Do not disable TLS verification. The server certificate includes `localhost`, `127.0.0.1`, and the selected bind IP. A localhost request validates the daemon/API on that machine only; it does **not** prove ESP32 TLS compatibility, LAN reachability, firewall behavior, or hardware media behavior.

## Tray and Settings lifecycle

The tray and HTTPS runtime stay alive independently of Settings. Startup creates no settings window or OpenGL renderer. Left-click opens Settings; right-click retains the Settings, pause/resume, and Quit menu. Closing Settings (including its X button) destroys the settings window and renderer; opening it again creates a fresh UI from the current configuration. Launching the executable again activates the existing agent's Settings instead of creating another listener.

For Windows acceptance, verify Quit and pause/resume before ever opening Settings, then open/close Settings repeatedly, restore it from minimized through the tray, and relaunch the executable both with Settings open and closed. Check HTTPS still works after closing Settings. Compare Task Manager private working set and GPU memory before opening, while open, and after closing; repeat to check for growth. Graphics drivers, the reusable event loop, and allocator caches may retain some memory; resource savings are not claimed until measured on Windows.

## Configuration and execution safety

- The token and TLS private keys are DPAPI-protected for the current Windows user. The trusted CA is public and is exported only through Settings. Changing the bind IP creates new TLS trust material; explicitly re-pair clients.
- Listener and startup preferences, bind address/port, and action mappings are stored under `%LOCALAPPDATA%\Esp32MediaDaemon`. Startup registration is per-user. Only one app instance is allowed per interactive session.
- Actions are configured locally and addressed remotely only by ID. Application launch passes an argument vector directly. `.bat`/`.cmd` actions use `cmd.exe /D /V:OFF /S /C`; due to cmd's expansion rules, batch paths/arguments are restricted to ASCII letters/digits, spaces and `._-/\:,+()[]=`. Quotes, `%`, `!`, shell metacharacters, and controls are rejected before spawn. PowerShell uses the selected absolute `powershell.exe` or `pwsh.exe` with `-NoProfile -NonInteractive -File`; execution policy is not bypassed.
- Local **Test** requires confirmation. Configured scripts run as the logged-in user, are not sandboxes, and are limited to one at a time with a 60-second process-tree timeout. Application launches are not supervised or killed. Script output is discarded; diagnostics retain only bounded request/action IDs and outcome categories.
- Pause closes the HTTPS listener; it does not undo an action already issued. There is no offline queue or automatic retry.

## Validation status

Linux host-side core/HTTPS tests and strict Clippy are available, but they are not a substitute for the Windows-only APIs. Windows target compilation, DPAPI/registry behavior, installer execution, tray interaction, real GSMTC/audio behavior, batch/PowerShell fixtures, and localhost API requests on Windows must be confirmed by the Windows CI runner and a Windows 11 user. ESP32/LAN interoperability and gaming performance are explicitly unverified/deferred.
