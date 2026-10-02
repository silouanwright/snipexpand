# Historical handoff

Superseded by the authorized [v0.5.0 integration](../../release-0.5.0.md) on
2026-10-02. The original checkout remains preserved. The restrictions and pending
results below describe the earlier checkpoint.

## Status clarification — 2026-10-01

The keyboard-access repair and plugin Restart feedback shipped separately as
v0.4.1. This Unicode/Fcitx5 development work remains unpublished on
`codex/fcitx5-direct-commit`; the checkout also contains copies of the shipped
permission changes that must be reconciled with main before the next release.
Local doctor checks pass and report the bridge loaded, but that does not replace
the pending current Chromium test or final Signal policy smoke test below.
See [TASKS.md](../../../TASKS.md) for the current ordered backlog.

## Decision

Implement a Fcitx5 addon as an opportunistic, flash-free direct-commit backend
for Chromium and Electron. Keep the existing Wayland keymap path for terminals
and numeric Unicode compose as a safe compatibility fallback.

## Current safety gate

Do not install or load the addon, refresh or restart Fcitx5, focus windows, mutate the clipboard, or inject live input without fresh explicit approval. Offline compilation and deterministic Rust/C++ tests are allowed.

## Implemented boundaries

- `fcitx5-addon/` contains the narrow Fcitx addon and standalone suffix tests.
- `src/fcitx5.rs` contains optional installation, bridge discovery, D-Bus
  result classification, and fail-safe retry policy.
- `src/daemon.rs` uses the bridge only for supplementary Unicode in detected
  Chromium and Electron applications. Terminals keep the persistent Wayland
  keymap. A stale suffix gets one exact retry. Missing surrounding text gets a
  100-millisecond fallback permit bound to the same Fcitx input context,
  trigger, and replacement. Safe refusals then use compose; password contexts
  are suppressed; an indeterminate dispatched call is never retried.
- `src/config.rs` exposes `non_bmp_input: fcitx5` as an explicit override.
  `fcitx_sensitive_hint: allow | suppress` controls Fcitx's distinct privacy
  hint globally or per profile; `Password` is always refused.
- `src/main.rs` integrates optional installation and diagnostics.
  Diagnostics distinguish a bridge that is present on disk from one Fcitx has
  actually loaded.

## Offline validation completed

- Fcitx addon compiled and its C++ test passed against Fcitx 5.1.21 on the
  Omarchy host.
- The same addon compiled and tested in an isolated Ubuntu 24.04 container
  against Fcitx 5.1.7.
- `cargo fmt --check`, `cargo test --locked` (91 unit and 5 integration tests),
  `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo run --locked -- --help` passed.
- `cargo audit` found no known vulnerable dependency and `cargo package
  --locked --list --allow-dirty` includes every runtime addon source.

## Guarded Signal result

The first guarded Signal test proved that Signal does not expose usable
surrounding text. It also showed that Signal marks its ordinary composer with
Fcitx's broader `Sensitive` privacy hint, which is distinct from `Password`.
After separating those flags and installing the guarded same-context fallback,
Signal 8.23.0 expanded `;sm` to 🙂 immediately without numeric preedit. The
daemon journal recorded `Fcitx5 committed the expansion without a Unicode
compose fallback` twice. Actual Fcitx `Password` contexts remain suppressed.

The current isolated Chromium target still needs a separately approved bridge
run. The new configurable sensitive-hint policy also requires one final addon
reload and Signal smoke test; its allow/suppress routing and permanent password
block are covered offline. This Unicode/Fcitx5 work has not been released or
pushed; the separate v0.4.1 permission repair is already published.
