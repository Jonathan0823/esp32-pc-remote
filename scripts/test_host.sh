#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for test in wol_packet voice_policy; do
  c++ -std=c++11 -Wall -Wextra -Werror \
    "$root/tests/${test}_test.cpp" -o "$tmp/$test"
  "$tmp/$test"
done
echo "Host tests passed"
