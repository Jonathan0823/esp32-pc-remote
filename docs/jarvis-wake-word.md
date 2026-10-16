# Jarvis wake word

The ESP32-S3 uses Espressif WakeNet model `wn9_jarvis_tts` for the always-listening trigger. Saying “Jarvis” opens the existing approximately 5.76-second MultiNet command window; the trigger alone does not send WoL or a media request. The command phrases and session behavior are unchanged.

The bundled [`models/srmodels.bin`](../models/srmodels.bin) contains Jarvis plus the existing English `mn7_en` command model and `vadnet1_medium` VAD model. Its source revision, configuration, license, size, and regeneration instructions are in [`models/README.md`](../models/README.md). The PlatformIO build hook checks the expected model names and partition size and attaches this image at the existing model partition offset (`0xC10000`).

Use a USB/serial full-image upload to install the model image:

```sh
pio run -t upload --upload-port /dev/ttyACM0
```

Replace the port with the board's serial port. App-only LAN OTA does not update the model partition. Devices that were flashed with the old `Hi ESP` image need this USB upload to receive the Jarvis model.

After flashing, check Serial for `[voice] listening for Jarvis`. Say “Jarvis” and confirm the blue command-listening indication starts without any request being sent; then say a valid command within the existing timeout. Recognition distance, noise tolerance, and false-trigger rate need to be checked with the actual microphone and environment.
