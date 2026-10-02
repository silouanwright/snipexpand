# Snippet authoring checkpoint — 2026-10-01

## Scope and state

Implementation commit: `fc90d52`.
Branch: `codex/snippet-authoring`, based on released main `4b8d649` (v0.4.1).
Checkout: `/home/silouan/Work/projects/snipexpand-authoring`.
The separate `codex/fcitx5-direct-commit` checkout retains the unfinished Unicode
work. This batch is implemented locally and has not been deployed or released.

## Result

- `render TRIGGER [--source PATH] [--profile NAME] [--json]` shares the real
  renderer and prints replacement text plus optional source/cursor metadata.
  Duplicate selection is explicit, profiles need no focused window, and the
  command does not create config files, contact IPC, or touch the clipboard.
- Date rendering returns errors rather than panicking on invalid format/offset
  input. Configuration validates parameters by variable type, including unused
  globals. Runtime errors cancel automatic expansion before deletion/injection.
- Date `tz` supports named IANA zones through Chrono-TZ; omitted means local.
  Nested dates share one instant. Offsets remain elapsed seconds. Unknown zones
  are rejected, a documented difference from Espanso's local-time fallback.
- Settings/match schemas are bundled and exportable through `schema config|match`.
  The editor guide explains local YAML Language Server associations and the
  remaining semantic checks performed by `check`.
- Updated CLI help, README, compatibility docs, package shortcut skill, and tasks.

## Verification

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `TMPDIR="$PWD/target/test-tmp" cargo test --locked`: 89 tests pass
  (77 unit, 7 authoring integration, 5 existing pack integration).
- CLI root/render help passes. Package listing contains both schema files, the
  editor guide, renderer modules, and test fixtures; package verification builds.
- First test attempt failed because system `/tmp` returned disk-quota errors.
  A checkout-local temporary directory resolved this; no system cleanup was done.

## Limits and next work

Preview evaluates literal insertion, not typed trigger matching. Regex examples,
word-boundary simulation, typed case propagation, and live application delivery
remain outside its scope. Schema checking is structural; date/regex validity and
cross-file references still require the CLI. Schema tests have no network/file
reference resolution, and the validator dependency is test-only.

No live keyboard tests, Fcitx reload, installed daemon replacement, version bump,
remote push, or release was performed. Review/merge this branch independently;
reconcile the unfinished Unicode branch with these renderer/config changes later.
Echo variables, locale overrides, and named personal groups remain follow-ups.

Documentation follow-up on 2026-10-02: see [the next-work plan](next-work-2026-10-02.md)
for the proposed generated-test suite, echo support, and groups. The 89-test
result above belongs to the implementation validation; it was not rerun for
this documentation-only update.

Subsequent implementation is tracked in the
[2026-10-02 reliability/echo checkpoint](checkpoint-reliability-2026-10-02.md).
