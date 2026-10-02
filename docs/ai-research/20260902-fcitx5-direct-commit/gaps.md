# Open gaps

- Applications that do not expose surrounding text use a short-lived,
  same-context Fcitx fallback. Signal 8.23.0 passed the guarded test with
  immediate emoji output, no numeric preedit, and journal confirmation of the
  Fcitx commit. A rejected guard retains the existing compose fallback.
- Isolated Chromium still needs a guarded run against the current bridge. Its
  earlier passing result exercised the superseded wtype implementation.
- The installer needs a guarded end-to-end check on stock Omarchy. Its optional
  failure path is deterministic, but has not been exercised on a host without
  Fcitx5 development headers.
- `snipexpand uninstall` currently removes the daemon service while preserving
  user data, including the optional per-user addon. Decide whether a future
  explicit full-removal command should also remove that addon.
- GNOME and non-Wayland environments must continue using the existing compatibility paths.
