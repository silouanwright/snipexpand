#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
expected="$repo_root/tests/compatibility/expected.txt"
actual=$(mktemp "${XDG_RUNTIME_DIR:-/tmp}/snipexpand-compatibility.XXXXXX")

command -v wl-copy >/dev/null
command -v wl-paste >/dev/null
command -v hyprctl >/dev/null
command -v jq >/dev/null

printf 'SNIPEXPAND_COMPATIBILITY_PENDING' | wl-copy
printf '%s\n' "Focus the empty target field now. The test starts in 3 seconds."
sleep 3

target_window=$(hyprctl activewindow -j)
target_address=$(jq -er '.address | select(length > 0)' <<<"$target_window")
target_class=$(jq -r '.class // "unknown"' <<<"$target_window")
target_title=$(jq -r '.title // "untitled"' <<<"$target_window")

SNIPEXPAND_E2E_EVENT_DELAY_MS="${SNIPEXPAND_E2E_EVENT_DELAY_MS:-15}" \
SNIPEXPAND_E2E_EXPANSION_PAUSE_MS="${SNIPEXPAND_E2E_EXPANSION_PAUSE_MS:-250}" \
  "$repo_root/target/debug/examples/e2e_type" --compatibility-copy &
driver_pid=$!

while kill -0 "$driver_pid" 2>/dev/null; do
  active_window=$(hyprctl activewindow -j)
  active_address=$(jq -r '.address // ""' <<<"$active_window")
  if [[ "$active_address" != "$target_address" ]]; then
    kill "$driver_pid" 2>/dev/null || true
    wait "$driver_pid" 2>/dev/null || true
    active_class=$(jq -r '.class // "unknown"' <<<"$active_window")
    active_title=$(jq -r '.title // "untitled"' <<<"$active_window")
    printf 'FAIL: focus left %s (%s) for %s (%s); result discarded\n' \
      "$target_title" "$target_class" "$active_title" "$active_class" >&2
    exit 1
  fi
  sleep 0.05
done
wait "$driver_pid"

active_window=$(hyprctl activewindow -j)
if [[ $(jq -r '.address // ""' <<<"$active_window") != "$target_address" ]]; then
  active_class=$(jq -r '.class // "unknown"' <<<"$active_window")
  active_title=$(jq -r '.title // "untitled"' <<<"$active_window")
  printf 'FAIL: focus left %s (%s) for %s (%s) before capture; result discarded\n' \
    "$target_title" "$target_class" "$active_title" "$active_class" >&2
  exit 1
fi

wl-paste --no-newline >"$actual"
if cmp --silent <(head -c -1 "$expected") "$actual"; then
  printf '%s\n' "PASS: clipboard content exactly matches expected.txt"
else
  diff --unified <(head -c -1 "$expected") "$actual" || true
  printf '%s\n' "FAIL: clipboard content differs; captured at $actual" >&2
  exit 1
fi
