# Source Ledger: Optional Wayland Input Method

## Scope Fence

Current lane: optional direct UTF-8 insertion through Wayland input-method-v2,
including application activation, exclusive seat ownership, and fallback.

Allowed roots:

- `/home/silouan/Work/snipexpand`
- Official Wayland, wayland-protocols, wayland-rs, Hyprland, Chromium/Electron,
  and wl-ime-type sources

Forbidden roots:

- Unrelated local repositories and user data
- Live desktop input, clipboard mutation, focus changes, or application launch

Out-of-scope fallback rule: record missing evidence instead of broadening the
search or testing against the live desktop without explicit user approval.

| Source | URL/path | Date | Tier | Relevance |
| --- | --- | --- | --- | --- |
| Wayland input-method-v2 protocol | https://wayland.app/protocols/input-method-unstable-v2 | 2026-09-02 | Primary | Defines exclusive seat ownership, activation, serials, direct commit, deletion, and unavailable behavior |
| wayland-protocols-misc 0.3.12 bindings | `/home/silouan/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/wayland-protocols-misc-0.3.12` | 2026-09-02 | Primary/local dependency | Confirms the existing dependency already generates input-method-v2 client bindings |
| wl-ime-type | https://gitlab.freedesktop.org/emersion/wl-ime-type | 2026-09-02 | Primary | Minimal reference client that waits for activation or unavailable, commits text, and flushes before exit |
| Hyprland input-method relay | https://github.com/hyprwm/Hyprland/blob/main/src/managers/input/InputMethodRelay.cpp | 2026-09-02 | Primary | Rejects a second input method and relays active text-input state to the registered owner |
| Hyprland text-input relay | https://github.com/hyprwm/Hyprland/blob/main/src/managers/input/TextInput.cpp | 2026-09-02 | Primary | Shows commit-string and delete-surrounding forwarding to text-input-v3 clients |
| Fcitx5 Wayland frontend | https://github.com/fcitx/fcitx5/tree/master/src/frontend/waylandim | 2026-09-02 | Primary | Confirms Fcitx5 itself occupies the Wayland input-method role |
| Fcitx5 D-Bus frontend | https://github.com/fcitx/fcitx5/blob/master/src/frontend/dbusfrontend/dbusfrontend.cpp | 2026-09-02 | Primary | Exposes commit strings as signals to the client that created a context, not as a global arbitrary-target insertion method |
| Local Fcitx5 controller and process inspection | `busctl --user`, `fcitx5-remote`, `ps` | 2026-09-02 | Primary/local observation | Fcitx5 is running and reports active Wayland contexts for Signal, Chromium, and Foot |
