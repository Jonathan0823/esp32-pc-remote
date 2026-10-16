#!/usr/bin/env python3
"""Build and upload the app image without exposing the OTA password in logs/argv."""

import argparse
import importlib.util
import ipaddress
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PASSWORD_PATTERN = re.compile(r"[0-9a-f]{64}\Z")
PASSWORD_DEFINE = re.compile(
    r'^[ \t]*#[ \t]*define[ \t]+PC_OTA_PASSWORD[ \t]+"([^"\r\n]*)"[ \t]*(?://[^\r\n]*)?\r?$',
    re.MULTILINE,
)


def parse_ota_password(config_text: str) -> str:
    logical_text = re.sub(r"\\\r?\n[ \t]*", " ", config_text)
    matches = PASSWORD_DEFINE.findall(logical_text)
    if len(matches) != 1 or not PASSWORD_PATTERN.fullmatch(matches[0]):
        raise ValueError("config.h must define PC_OTA_PASSWORD once as 64 lowercase hex characters")
    return matches[0]


def load_espota(core_dir: Path):
    uploader = core_dir / "packages/framework-arduinoespressif32/tools/espota.py"
    if not uploader.is_file():
        raise FileNotFoundError("PlatformIO's bundled espota.py was not found")
    spec = importlib.util.spec_from_file_location("pio_espota", uploader)
    if spec is None or spec.loader is None:
        raise RuntimeError("Could not load PlatformIO's bundled espota.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_espota(espota, device_ip: str, password: str, firmware: Path) -> int:
    return int(espota.main(["--ip", device_ip, "--auth", password, "--file", str(firmware), "--progress"]))


def main() -> int:
    parser = argparse.ArgumentParser(description="Build and upload firmware over LAN OTA")
    parser.add_argument("--ip", help="ESP32 IP address (or set OTA_IP)")
    args = parser.parse_args()
    raw_ip = args.ip or os.environ.get("OTA_IP")
    if not raw_ip:
        parser.error("set OTA_IP, for example: make ota OTA_IP=192.168.1.50")
    try:
        device_ip = str(ipaddress.ip_address(raw_ip))
    except ValueError:
        parser.error("--ip must be an IPv4 or IPv6 address")

    try:
        password = parse_ota_password((ROOT / "config.h").read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        print(f"OTA configuration error: {exc}", file=sys.stderr)
        return 2

    build = subprocess.run(["pio", "run", "-e", "esp32-s3"], cwd=ROOT, check=False)
    if build.returncode:
        return build.returncode

    firmware = ROOT / ".pio/build/esp32-s3/firmware.bin"
    if not firmware.is_file():
        print("OTA build did not produce firmware.bin", file=sys.stderr)
        return 2

    try:
        info = subprocess.run(
            ["pio", "system", "info", "--json-output"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        core_dir = Path(json.loads(info.stdout)["core_dir"]["value"])
        espota = load_espota(core_dir)
    except (FileNotFoundError, KeyError, json.JSONDecodeError, subprocess.CalledProcessError, RuntimeError) as exc:
        print(f"Could not locate PlatformIO's OTA uploader: {exc}", file=sys.stderr)
        return 2

    if not sys.stdin.isatty():
        print("Run make ota from an interactive terminal", file=sys.stderr)
        return 2
    input("Build complete. Hold BOOT for 3 seconds, release it, then press Enter to upload.")

    # Invoke the pinned uploader in-process without --debug, which dumps its arguments.
    return run_espota(espota, device_ip, password, firmware)


if __name__ == "__main__":
    sys.exit(main())
