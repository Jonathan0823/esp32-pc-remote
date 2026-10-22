# Hardware validation

This developer acceptance checklist is separate from the [user quickstart](../README.md#quickstart-wake-your-pc-by-voice). Perform it only on hardware and a Windows PC you are authorized to use. Use disposable credentials for fixtures; do not use production tokens in tests. These are checks to perform, not a record of completed validation.

- Confirm Serial prints `[voice] listening for Jarvis` after successful microphone/ESP_SR initialization.
- Say “Jarvis”; verify a blue listening indication and no WoL packet yet.
- Say “wake” within about 5.76 seconds; verify one WoL submission and a green RGB flash lasting about 500 ms.
- Let a listening window time out; verify the LED turns off, wake-word mode resumes, and no packet is sent.
- Start a new session immediately after command completion; verify another command is accepted without a five-second cooldown.
- Disconnect Wi-Fi, say “Jarvis” and “wake”; verify a red flash and no delayed WoL after reconnecting.
- Submit a confirmed Telegram wake and an MQTT wake; verify a purple Telegram flash and cyan MQTT flash, each lasting about 500 ms.
- Verify a request waiting for confirmation does not flash a submission color.
- Check voice logs distinguish listening start/timeout, media/custom action queued/accepted/rejected/unknown, sync activation/acknowledgment, and WoL packet submission; verify Telegram and MQTT/dashboard controls still work.
- With disposable settings and the LAN HTTPS test fixture, try all six media phrases; verify one matching accepted request/yellow pulse per session. Stop the fixture or use an untrusted certificate; verify red feedback and no retry after reconnect. The fixture does not execute media, so white execution feedback awaits the Windows daemon.
- On a paired ESP and Windows daemon, configure a disposable custom phrase, save it without applying, then apply and verify Settings remains pending until “Jarvis, update”. Confirm the custom phrase dispatches exactly once; reboot and verify the active phrase remains; test unavailable daemon, rejected phrase-table update, and action edits during a stale published revision. Do not flash or run actions without explicit permission.
