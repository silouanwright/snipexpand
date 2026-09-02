#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
profile=${SNIPEXPAND_CHROMIUM_PROFILE:-${XDG_CACHE_HOME:-$HOME/.cache}/snipexpand/chromium-compatibility}

command -v chromium >/dev/null

if pgrep -x chromium >/dev/null; then
  printf '%s\n' "Warning: normal Chromium is running, so any 1Password prompt cannot be attributed to this profile." >&2
fi

mkdir -p "$profile"

printf 'Profile: %s\n' "$profile"
printf '%s\n' "Stage 1 only: wait and confirm that no 1Password window appears."
printf '%s\n' "Do not run the typing verifier until this browser remains stable."

exec chromium \
  --user-data-dir="$profile" \
  --disable-extensions \
  --password-store=basic \
  --no-first-run \
  --no-default-browser-check \
  --disable-sync \
  --new-window \
  "file://$repo_root/tests/compatibility/target.html"
