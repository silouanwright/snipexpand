#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
expected="$repo_root/tests/compatibility/expected.txt"
actual=${1:-${XDG_CACHE_HOME:-$HOME/.cache}/snipexpand/compatibility/nvim-clean.txt}

command -v hyprctl >/dev/null
command -v jq >/dev/null
[[ -f "$actual" ]] || {
  printf 'Target file does not exist: %s\n' "$actual" >&2
  exit 1
}

printf '%s\n' "Focus the empty target buffer now. The test starts in 3 seconds."
sleep 3

target_window=$(hyprctl activewindow -j)
target_address=$(jq -er '.address | select(length > 0)' <<<"$target_window")
target_class=$(jq -r '.class // "unknown"' <<<"$target_window")
target_title=$(jq -r '.title // "untitled"' <<<"$target_window")

SNIPEXPAND_E2E_EVENT_DELAY_MS="${SNIPEXPAND_E2E_EVENT_DELAY_MS:-15}" \
SNIPEXPAND_E2E_EXPANSION_PAUSE_MS="${SNIPEXPAND_E2E_EXPANSION_PAUSE_MS:-250}" \
  "$repo_root/target/debug/examples/e2e_type" --compatibility &
driver_pid=$!

while kill -0 "$driver_pid" 2>/dev/null; do
  active_window=$(hyprctl activewindow -j)
  active_address=$(jq -r '.address // ""' <<<"$active_window")
  if [[ "$active_address" != "$target_address" ]]; then
    if hyprctl clients -j | jq -e --arg address "$target_address" \
      'any(.[]; .address == $address)' >/dev/null; then
      kill "$driver_pid" 2>/dev/null || true
      wait "$driver_pid" 2>/dev/null || true
      active_class=$(jq -r '.class // "unknown"' <<<"$active_window")
      active_title=$(jq -r '.title // "untitled"' <<<"$active_window")
      printf 'FAIL: focus left %s (%s) for %s (%s); result discarded\n' \
        "$target_title" "$target_class" "$active_title" "$active_class" >&2
      exit 1
    fi
    # The driver closes clean Neovim with :wq. Focus may move after the target
    # disappears but before the driver process finishes its final pause.
    break
  fi
  sleep 0.05
done
wait "$driver_pid"

if cmp --silent "$expected" "$actual"; then
  printf '%s\n' "PASS: target file exactly matches expected.txt"
else
  diff --unified "$expected" "$actual" || true
  printf 'FAIL: target file differs: %s\n' "$actual" >&2
  exit 1
fi
