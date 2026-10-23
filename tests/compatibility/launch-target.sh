#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
target=${1:-}
cache_root=${SNIPEXPAND_COMPATIBILITY_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/snipexpand/compatibility}

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
  firefox)
    require firefox
    profile="$cache_root/firefox-profile"
    mkdir -p "$profile"
    cat > "$profile/user.js" <<'PREFS'
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
PREFS
    exec env MOZ_ENABLE_WAYLAND=1 firefox --no-remote --profile "$profile" \
      "file://$repo_root/tests/compatibility/target.html"
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
    mkdir -p "$cache_root/zed-profile/config"
    # Only the guarded VM runner enables trust for this synthetic fixture profile.
    fixture_trust=false
    if [[ ${SNIPEXPAND_VM_FIXTURE:-} == 1 ]]; then fixture_trust=true; fi
    printf '{"vim_mode":false,"autosave":"off","show_welcome":false,"disable_ai":true,"auto_indent":false,"use_autoclose":false,"session":{"trust_all_worktrees":%s}}\n' "$fixture_trust" > "$cache_root/zed-profile/config/settings.json"
    exec zeditor \
      --new \
      --wait \
      --user-data-dir "$cache_root/zed-profile" \
      "$target_file"
    ;;
  *)
    printf '%s\n' "Usage: tests/compatibility/launch-target.sh chromium|firefox|electron|gtk|nvim|qt|zed" >&2
    exit 2
    ;;
esac
