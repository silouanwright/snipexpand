# Reliability and echo checkpoint — 2026-10-02

## Active goal status

The goal is still active. Generated reliability tests, reproduced fixes, and
bounded echo variables are done. Named personal groups are not implemented yet.
Continue with [the group contract](group-contract.md), then verify the complete
goal against its original requirements before marking it complete.

Checkout: `/home/silouan/Work/projects/snipexpand-authoring`.
Branch: `codex/snippet-authoring`, based on v0.4.1 and authoring commit `fc90d52`.

Local commits:

- `cd406fa`: generated matcher suite; fixes empty-buffer growth, case collisions,
  and lost whole-word boundaries.
- `1c40794`: bounded echo variables, deterministic single-pass substitution,
  dependency validation, escaping, graph/schema/CLI fixtures, rendering budgets.
- `e2a85a8`: rollback every completed pack-update move on failure; preserve
  recovery backups on rollback failure; reload cancels stale queued input and
  reports validation errors over IPC to the CLI.

Nothing was pushed, installed, released, or tested through live keyboard input.
The separate unfinished Unicode checkout and live desktop setup are preserved.

## Verification

111 tests pass: 90 unit, 15 authoring CLI, 6 pack CLI. Formatting, Clippy with
warnings denied, CLI help, and `git diff --check` passed before the latest code
commit. Extended seed `20261002`: 60,000 matcher cases (8.05s), 6,000 graph/schema
cases (11.59s). Graph smoke and the branching evaluation regression also passed
under a 2 GiB virtual-memory cap and 30-second process deadlines.

Use `TMPDIR="$PWD/target/test-tmp"` for tests: system `/tmp` has quota errors.
Detailed evidence and reproduction commands are in
[the campaign report](testing/generated-suite.md).

## Implementation notes for continuing

- `src/template.rs` resolves echo dependencies, parses escaped braces, and tracks
  cumulative byte/evaluation limits. Echo parameter references are strict;
  unknown placeholders in replacements retain their legacy literal behavior.
- `Config::validate_match_references` now validates depth as well as cycles,
  caching subtree heights so declaration order cannot bypass the 64-node limit.
- `reload_matching_config` in `src/daemon.rs` loads a candidate and atomically
  replaces pure matching state; hardware effects are applied by its caller.
  Groups need an analogous install-exact-candidate path after persistence.
- `swap_pack_paths` in `src/packs.rs` has injected rename operations for fault
  tests. Recovery backups are retained and reported if rollback cannot finish.
- Tests always use temporary config/data roots. Pack CLI harness now explicitly
  overrides `XDG_RUNTIME_DIR` as well.

## Remaining checks and delivery

Implement configuration/CLI/IPC/state for groups, update documentation and task
handoffs, run the full suite once new changes are in place, and commit locally.
Reconcile remaining documentation-only changes from the preceding planning pass.
Package verification can be repeated at the final batch boundary; it was last
run for `fc90d52`, before these new modules. Do not claim that offline tests
complete the open Unicode/Fcitx application matrix.
