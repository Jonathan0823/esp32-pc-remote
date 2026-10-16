#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for test in wol_packet voice_policy indicator_policy media_control_policy ota_policy; do
  c++ -std=c++11 -Wall -Wextra -Werror \
    "$root/tests/${test}_test.cpp" -o "$tmp/$test"
  "$tmp/$test"
done
c++ -std=c++11 -Wall -Wextra -Werror \
  -DPC_MEDIA_HOST='"192.168.1.50"' -DPC_MEDIA_PORT=8443 \
  -DPC_MEDIA_CA_CERT='"test-ca"' \
  -DPC_MEDIA_API_TOKEN='"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"' \
  "$root/tests/media_control_config_enabled_test.cpp" -o "$tmp/media_config_enabled"
"$tmp/media_config_enabled"
c++ -std=c++11 -Wall -Wextra -Werror \
  -DPC_OTA_PASSWORD='"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"' \
  "$root/tests/ota_config_enabled_test.cpp" -o "$tmp/ota_config_enabled"
"$tmp/ota_config_enabled"
PYTHONPATH="$root${PYTHONPATH:+:$PYTHONPATH}" python3 -m unittest discover -s "$root/tests" -p 'test_*.py'
echo "Host tests passed"
