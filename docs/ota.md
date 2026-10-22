# LAN app-firmware OTA

OTA is disabled unless `PC_OTA_PASSWORD` is configured. It updates only the ESP32 application image over the existing Wi-Fi LAN; it does not update speech models, the filesystem, bootloader, or partition table. USB flashing and the existing model-image workflow are unchanged.

## Configure

1. Generate a unique per-device credential with `openssl rand -hex 32`.
2. Put it in the local, untracked `config.h`:

   ```cpp
   #define PC_OTA_PASSWORD "<64 lowercase hex characters>"
   ```

3. **Before the first `make ota` upload**, build and flash this OTA-enabled firmware over USB using the [README's USB flashing instructions](../README.md#3-build-and-flash-over-usb). Configure `PC_OTA_PASSWORD` first: the USB-flashed firmware must include that credential, matching the value the uploader reads from your local `config.h`. Editing `config.h` alone does not enable OTA on an already-flashed board.
4. Let the board boot and connect to Wi-Fi, then find its current IP address in the router's DHCP client list. OTA does not advertise mDNS.

## Upload

From the project directory, run this with the board's IP:

```bash
make ota OTA_IP=192.168.1.50
```

Replace `192.168.1.50` with the board IP. The target builds first, then prompts you to hold the physical BOOT button for 3 seconds, release it, and press Enter to upload. This opens the 2-minute window only after the build is complete. The uploader reads the password directly from `config.h`; it is not entered on the command line or printed in upload logs. It invokes PlatformIO's bundled ESP OTA uploader without debug-argument logging and sends only `firmware.bin`, not the extra speech-model image used by USB flashing.

Dim amber means the window is ready, dim cyan means an app transfer is active, green means the image validated, and red means activation or update failed. A BOOT hold during startup is ignored until the button is released. A fresh hold is required after a failed or expired window.

The window requires Wi-Fi and a valid credential. Upload admission closes when Wi-Fi disconnects or the window expires. Once a valid app transfer starts, it may finish after the 2-minute admission window, but has a 180-second total deadline and the framework's bounded idle timeout. Temporary loss of the voice, WoL, MQTT, and Telegram services during transfer is expected.

## Security and recovery

This is password-authenticated ArduinoOTA, not TLS. The firmware transfer is plaintext and this feature does not provide signed-image verification; challenge-response authentication does not encrypt traffic. A LAN attacker may observe or alter firmware traffic. Use only on a trusted, private LAN; never port-forward the OTA port or expose it to the Internet. The credential must be distinct from all service tokens.

An interrupted or invalid transfer should leave the currently running app selected because the new app slot is activated only after the framework validates the completed image. A fully installed but faulty app is **not** guaranteed to roll back automatically. Recover through the existing USB/serial flashing procedure in the README; its PlatformIO extra-script may also flash the configured speech-model image. Do not interrupt power during a flash.

**Hardware check still required:** confirm the actual board's BOOT switch is connected to active-low GPIO0 and test success, interrupted-transfer recovery, and USB recovery on the device before relying on OTA.
