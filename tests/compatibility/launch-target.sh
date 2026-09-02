#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
target=${1:-}
cache_root=${XDG_CACHE_HOME:-$HOME/.cache}/snipexpand/compatibility

require() {
  command -v "$1" >/dev/null || {
    printf 'Missing required command: %s\n' "$1" >&2
    exit 1
  }
}

case "$target" in
  chromium)
    exec "$repo_root/tests/compatibility/launch-chromium.sh"
    ;;
  electron)
    require electron43
    mkdir -p "$cache_root/electron"
    exec electron43 \
      --user-data-dir="$cache_root/electron" \
      --disable-extensions \
      --password-store=basic \
      "$repo_root/tests/compatibility/target.html"
    ;;
  gtk)
    require zenity
    exec zenity \
      --text-info \
      --editable \
      --title="SnipExpand compatibility target" \
      --width=900 \
      --height=700 \
      --font=monospace </dev/null
    ;;
  qt)
    require qml6
    exec qml6 "$repo_root/tests/compatibility/target.qml"
    ;;
  nvim)
    require foot
    require nvim
    mkdir -p "$cache_root"
    target_file="$cache_root/nvim-clean.txt"
    : >"$target_file"
    printf 'Target file: %s\n' "$target_file"
    exec foot \
      --app-id snipexpand-compatibility \
      nvim --clean -n "$target_file" -c startinsert
    ;;
  zed)
    require zeditor
    mkdir -p "$cache_root/zed-profile"
    target_file="$cache_root/zed-target.txt"
    : >"$target_file"
    exec zeditor \
      --new \
      --wait \
      --user-data-dir "$cache_root/zed-profile" \
      "$target_file"
    ;;
  *)
    printf '%s\n' "Usage: tests/compatibility/launch-target.sh chromium|electron|gtk|nvim|qt|zed" >&2
    exit 2
    ;;
esac
