# SnipExpand v0.5.0 — integration and verification

## Included changes

- Offline snippet preview, cursor metadata, editor schemas, and checked date
  formatting with IANA timezones.
- Bounded echo variables with dependency validation and literal escaping.
- Persistent personal groups, CLI/IPC controls, profile intersection, and native
  Omarchy plugin switches. The picker hides group-disabled snippets.
- Generated matcher regression tests; fixes for empty matcher memory growth,
  case collisions, whole-word boundaries, pack rollback, and stale reload input.
- Application-aware supplementary Unicode delivery: optional Fcitx5 direct
  commits for Chromium/Electron, paced compose fallback, and persistent Wayland
  keymaps elsewhere. Addon diagnostics verify the exact embedded source build.

The Unicode checkout was preserved at its original branch and working state.
A full source snapshot and live binary/config backups are stored locally in
`/home/silouan/Documents/ChatGPT/SnipExpand/integration-backup-2026-10-02`.
Integration commit: `3dde657`. Plugin implementation: `9c5bb08`.

## Automated verification

149 Rust tests, Clippy with warnings denied, formatting, CLI help, and the C++
bridge build/test pass. The schema includes all integrated Unicode settings.
The plugin passes manifest validation, locked dependency verification, JS
model tests, and a real Quickshell Process test for group changes, refresh,
and error recovery. Light/dark native group views were rendered and inspected,
as were loading, empty, error, and unsupported-version states. The installed
shell was restarted after updates settled to discard a confirmed stale panel;
the refreshed installed panel now displays Groups and passes Tab/Space navigation
to its empty state and Escape navigation back and out. Real group mutations
are covered by the Quickshell Process test.

[Ubuntu CI passed](https://github.com/silouanwright/snipexpand/actions/runs/36974279043)
for released daemon commit `1cb979e`, including the C++ bridge and all Rust tests. Package
verification built the actual 60-file crate in a separate target directory.

## Live matrix — 2026-10-02

Omarchy 4.0.4, Hyprland 0.56.2, Fcitx5 5.1.22. Each complete fixture used
15 ms key events and a 250 ms expansion pause. Every run guarded the exact
focused window and restored the normal service afterward.

| Target | Result |
| --- | --- |
| GTK: Zenity 4.2.2 | Full byte-exact fixture passed |
| Qt 6.11.2 | Full byte-exact fixture passed |
| Chromium 152.0.7977.82 | Full byte-exact fixture passed; Fcitx direct commit confirmed in daemon log |
| Electron 43.6.0 | Full byte-exact fixture passed; Fcitx direct commit confirmed in daemon log |
| Neovim 0.12.5 clean in Foot 1.28.0 | Full byte-exact file fixture passed |
| Signal 8.26.0 | Non-BMP expansion passed in an unsent Note to Self draft with both `allow` and `suppress`; Fcitx direct commit confirmed; draft cleared |
| Firefox / Zed | Not installed; untested |

The full fixture covers ASCII punctuation, non-Latin and non-BMP Unicode,
multiline replacement, cursor placement, undo, and 20 consecutive expansions.
Two launch-only attempts stopped before input while adapting the harness to
Hyprland's Lua dispatcher and asynchronous focus change. Those attempts are
not compatibility results.

Signal committed the emoji under both policies: its composer did not trigger
the bridge's sensitive-hint suppression branch. This verifies Signal delivery
under both settings, but does not demonstrate live sensitive-field blocking;
that guard has deterministic C++ coverage. No message was sent.

The v0.5.0 daemon is installed locally and doctor reports all checks passing,
including keyboard access, service readiness, and the matching Fcitx bridge.
The installed plugin is at release commit `eb09407`.

## Published artifacts

- [Daemon v0.5.0](https://github.com/silouanwright/snipexpand/releases/tag/v0.5.0),
  tagged at `1cb979e`.
- [Omarchy plugin v0.5.0](https://github.com/silouanwright/snipexpand-omarchy/releases/tag/v0.5.0),
  tagged at `eb09407`.
- [Release pipeline](https://github.com/silouanwright/snipexpand/actions/runs/36974280790)
  passed both architecture builds, GitHub publication, and crates.io publication.
- Both downloaded Linux binaries matched their published SHA-256 files. The
  downloaded x86_64 binary reports `snipexpand 0.5.0`; the aarch64 binary was
  checksum-verified but not executed on this x86_64 host.
- [crates.io 0.5.0](https://crates.io/crates/snipexpand/0.5.0) is present in the
  registry index and its downloaded archive matches the index checksum:
  `992d108e0875de19a6817537e843964b9603372a26e5fde1fcc1cd77fba1d797`.

Release work is complete. Further routine graphical tests use the isolated
Omarchy VM documented in the local compatibility guide.

## Scope limits

Generated and offline tests do not prove application transport correctness.
Live results apply to the recorded application/toolkit versions and timings.
Firefox and Zed are not installed on this test host. Non-US keyboard layouts,
hotplug/resume campaigns, and an AUR package remain separate backlog items.
Fcitx password/sensitive-hint guards apply to the direct-commit path; SnipExpand
still observes global keyboard events and does not infer browser field types.
