#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_LOG="$(mktemp)"
trap 'rm -f "$TMP_LOG"' EXIT

cd "$ROOT"

arduino-cli compile --fqbn esp32:esp32:esp32 . >"$TMP_LOG" 2>&1

cache_db="$(find "$HOME/.cache/arduino/sketches" -name compile_commands.json -printf '%T@ %p\n' | sort -nr | head -1 | cut -d' ' -f2-)"
if [[ -z "${cache_db:-}" ]]; then
  echo "compile_commands.json not found in Arduino cache" >&2
  exit 1
fi

cp "$cache_db" compile_commands.json

python3 - <<'PY'
import json
import pathlib
import re

root = pathlib.Path.cwd().as_posix()
p = pathlib.Path("compile_commands.json")
data = json.loads(p.read_text())
cache_prefix = f"{pathlib.Path.home().as_posix()}/.cache/arduino/sketches/"
pat = re.compile(rf"^{re.escape(root)}/[0-9A-Fa-f]{{32}}/sketch/")

for entry in data:
    file = entry.get("file", "")
    file = file.replace(cache_prefix, f"{root}/")
    file = pat.sub(f"{root}/", file)
    if file.endswith(".ino.cpp"):
        file = file[:-8] + ".ino"
    entry["file"] = file

p.write_text(json.dumps(data, indent=2) + "\n")
PY

echo "updated compile_commands.json"
