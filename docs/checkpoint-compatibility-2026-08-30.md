# Compatibility checkpoint: 2026-08-30

> Update on 2026-09-02: Signal later exposed intermittent numeric Unicode
> preedit (`U+1f642`) with the short-lived wtype compose fallback. The current
> worktree replaces that path with a paced sequence owned by the persistent
> Wayland injector. The historical Chromium pass below predates this change;
> guarded Chromium and Signal retests remain required.

## Scope and safety boundary

This checkpoint resumes the v0.4.0 validation goal without publishing changes.
No further live keystroke injection, window focusing, or application typing is
allowed until the user gives fresh explicit approval.

`origin/main` is now `cd7003e`, an independent compatible `Cargo.lock` refresh.
This dirty compatibility work remains based on the v0.4.0 commit `47633fd` and
is intentionally one commit behind. Integrate the lock refresh when preparing
the product commit without folding it into the compatibility changes.

The recorded Omarchy environment is Omarchy 4.0.0-1, Hyprland 0.56.2,
Chromium 151.0.7922.137, Zed 1.14.2, Electron 43.4.0, Zenity 4.2.2 with GTK
4.22.4, Qt QML Runtime 6.11.1, Foot 1.27.0, Neovim 0.12.4, `wtype` 0.4-2,
and `wl-clipboard` 2.3.0. Firefox is not installed.

## Confirmed evidence

- The published v0.4.0 x86_64 artifact and checksum matched.
- A second isolated download of the published v0.4.0 x86_64 artifact passed
  its published SHA-256 checksum and reported `snipexpand 0.4.0`.
- Uninstalling and reinstalling preserved the existing configuration.
- `snipexpand install` had an IPC readiness race. The local fix waits up to
  three seconds for the restarted daemon before reporting success.
- Neovim `--clean` in Foot passed the byte-exact compatibility fixture using
  the direct Wayland text keymaps.
- Unicode keymaps rebuilt successfully after configuration reload in the
  existing live baseline.
- Chromium 151 converted `🧐` from U+1F9D0 to U+F9D0 through its direct Wayland
  text-keymap path. A manual Unicode compose sequence previously produced the
  correct code point.
- The two automated Chromium compose attempts are invalid. 1Password stole
  focus, and the captured text could not be attributed to Chromium.
- The persistent extension-free Chromium profile subsequently passed a
  launch-only control alongside the normal browser: after ten seconds the test
  target remained focused, no 1Password window existed, and the 1Password
  extension was absent from the test profile. No text was injected.
- The first focus-guarded typing run had no focus change and passed every case
  except the non-BMP portion of the Unicode line. The daemon log identified a
  deterministic argument bug: `wtype` rejects `-d 0`. The local builder now
  omits the delay option when the configured delay is zero.
- The later guarded Chromium 151 retest passed. The extension-free target held
  focus for ten seconds with no 1Password window, then the complete fixture
  matched byte-for-byte, including `🧐`. The daemon reported no injection
  error, confirming the zero-delay compose fix live.
- A clean-Neovim rerun also matched byte-for-byte and exited successfully. Its
  first guard reported a false focus change because the driver intentionally
  closed Foot with `:wq` before its final pause ended, causing focus to fall
  back to Signal. The reusable file verifier now distinguishes expected target
  closure from focus theft while the target still exists.
- 1Password's current log says global shortcut registration is unavailable. It
  also records browser-integration connections and authentication or Quick
  Access activity around the invalid runs. The normal integrated browser was
  still running, so those events cannot be attributed to the isolated Chromium
  profile and do not prove or disprove modifier leakage.

## Current local changes

- `src/main.rs` waits for daemon readiness after service installation.
- `examples/e2e_type.rs` can copy the generic compatibility result instead of
  issuing Neovim commands.
- `tests/compatibility/target.html` provides a browser and Electron text target.
- `tests/compatibility/target.qml` provides a Qt text target.
- `tests/compatibility/launch-chromium.sh` starts a persistent extension-free
  Chromium profile and warns when the normal browser makes prompt attribution
  ambiguous.
- `tests/compatibility/launch-target.sh` starts isolated Zed and Electron
  profiles or disposable GTK and Qt targets without typing into them.
- `tests/compatibility/verify-clipboard.sh` performs byte-exact clipboard
  verification and aborts if the active Hyprland window changes.
- `tests/compatibility/verify-file.sh` performs the same focus guarding for
  clean Neovim, permits its expected `:wq` closure, and compares the saved file
  byte-for-byte.
- `src/app.rs`, `src/config.rs`, `src/daemon.rs`, and `src/injector.rs` select
  non-BMP input per application:
  - `auto` uses a paced, injector-owned Unicode compose sequence for detected
    Chromium and Electron processes.
  - `keymap` preserves direct persistent Wayland keymaps.
  - `compose` lets an application profile opt an Electron or unusual client in.
- The compose sequence resolves all keys before injection, releases Ctrl and
  Shift before hexadecimal entry, waits around preedit and commit, and clears
  synthetic modifiers after both successful and failed dispatch.
- If compose-mode injection fails after emitting a direct-keymap prefix, the
  daemon reports the error without replaying the whole string and duplicating
  that prefix.
- Signal rendered `🙂` as a private-use bar glyph through the direct keymap,
  confirming that Electron shares Chromium's non-BMP truncation. Automatic
  detection recognizes generic Electron runtime artifacts instead of an
  application-name list. Signal later exposed an intermittent uncommitted
  `U+1f642` preedit in the former wtype compose path. Signal's guarded retest of
  the injector-owned replacement remains pending.

## Non-interactive validation

- 72 Rust unit tests passed.
- 5 pack CLI integration tests passed.
- Clippy passed for all targets and features with warnings denied.
- The compatibility example builds successfully.
- The clipboard verifier passes Bash syntax validation.
- The isolated Chromium launcher passes Bash syntax validation without being
  executed.
- The shared Zed, Electron, GTK, and Qt launcher passes Bash syntax validation
  without being executed.
- The driver, YAML fixture, and expected output were audited one-to-one. They
  cover ASCII punctuation, non-Latin and non-BMP Unicode, multiline text,
  cursor placement followed by typing, immediate undo back to `;undo `, and 20
  consecutive expansions. Reload uses a separate byte-exact crab-character
  fixture.
- A disposable `snipexpand init` run created the documented
  `non_bmp_input: auto` starter setting without touching the user config.
- The active user service still runs `~/.local/bin/snipexpand`; the worktree
  binary was not installed over it.
- The released Omarchy plugin at commit `966dda8` passed `qmlpack verify`, its
  JavaScript model checks, and Omarchy's manifest validator.
- Omarchy 4.0.0's stock `plugin add` and `plugin enable` commands cloned,
  validated, discovered, enabled, and placed that exact plugin commit under a
  disposable HOME. Only the shell IPC endpoint was replaced with assertions,
  so the installed panel and its placement were not changed.

## Launch and release decision

- The Reddit worksheet now reflects v0.4.0's packs, application profiles,
  regex and nested matches, pause controls, and Omarchy plugin. Its privacy
  claim is scoped to the daemon because explicit pack commands contact Git
  remotes.
- Do not advertise general graphical-application compatibility yet. The
  current worktree has byte-exact Foot, clean-Neovim, and isolated Chromium 151
  evidence; Zed, Electron, GTK, Qt, and Firefox remain pending. Public v0.4.0
  does not contain the Chromium compose fix.
- v0.4.1 should be a focused reliability release containing the
  service-readiness and Chromium-engine non-BMP fixes. Chromium is
  live-confirmed; Signal must pass before release.
- Keep the next feature release behind the P0 compatibility matrix. Named
  snippet groups are the provisional v0.5.0 candidate because they extend the
  existing pack, pause, IPC, and plugin controls without adding sensitive input
  capture. Revisit that choice when real user feedback exists.

## Still pending

1. Run byte-exact checks for Signal, Zed, Electron, GTK, and Qt. Firefox is not
   currently installed.
2. Complete the matrix, then turn the prepared Reddit worksheet into the final
   post and confirm the provisional next-feature priority.
3. Re-run the complete release validation before deciding whether these changes
   warrant v0.4.1.

The compatibility and product changes in this checkpoint are preserved on
`codex/wayland-unicode-compatibility`. Nothing has been pushed, tagged, or
released.
