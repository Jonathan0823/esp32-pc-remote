#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

pio run -t compiledb

test -s compile_commands.json || {
  echo "PlatformIO did not generate compile_commands.json" >&2
  exit 1
}
echo "updated compile_commands.json"
