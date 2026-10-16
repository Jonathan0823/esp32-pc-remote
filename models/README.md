# ESP-SR model image

`srmodels.bin` is generated with Espressif ESP-SR 2.2.2, source commit `6ab0744f2fcb9ab7dae8488bd2a24d2acc8ffdbc`, using [`sdkconfig`](sdkconfig) and the upstream `model/movemodel.py` script. It contains `wn9_jarvis_tts`, `mn7_en`, and `vadnet1_medium`; the previous `wn9_hiesp` model is intentionally omitted.

To regenerate, obtain that exact ESP-SR source revision and run from the repository root:

```sh
python3 /path/to/esp-sr/model/movemodel.py \
  -d1 models/sdkconfig \
  -d2 /path/to/esp-sr \
  -d3 /tmp/esp-sr-build
cp /tmp/esp-sr-build/srmodels/srmodels.bin models/srmodels.bin
```

The generated image is 3,340,322 bytes and fits the existing 0x3E0000-byte model partition. SHA-256: `0c08d529116533fdfbd676b2d4f1a34a0327e8653a563875b48b212c125296d8`.

The image is flashed by the regular PlatformIO image path to the existing `model` partition at `0xC10000`; app-only OTA does not update it. This model bundle is distributed under Espressif's MIT license; see [`LICENSE`](LICENSE). Recognition quality and false-trigger rate still require testing on the actual microphone and board.
