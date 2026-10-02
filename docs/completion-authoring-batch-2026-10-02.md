# Completed authoring batch — 2026-10-02

## Result

The active batch is complete on `codex/snippet-authoring` in
`/home/silouan/Work/projects/snipexpand-authoring`. Work remains local; no publishing,
installation, compositor restart, clipboard operation, or live input was performed.

| Commit | Reviewable change |
| --- | --- |
| `cd406fa` | Generated matcher tests and fixes for empty buffers, case collisions, and whole-word boundaries |
| `1c40794` | Bounded echo variables, deterministic dependencies/escaping, graph/schema/output tests |
| `e2a85a8` | Pack-update rollback and reload input-state/error-reporting fixes |
| `ec9560a` | Research/planning records and interim handoff |
| `5d5bb14` | Persistent personal groups: configuration, selection, CLI, IPC, failure handling, docs, and tests |

These build on `fc90d52` (preview, schemas, and timezone dates) and released
v0.4.1. The package version has not been bumped. Plugin group buttons, deployment,
Unicode/Fcitx reconciliation, and the live application matrix remain separate
follow-ups; they are not requirements of this offline batch.

## Requirement-by-requirement audit

| Goal requirement | Current evidence |
| --- | --- |
| Reproducible generated compatibility/reliability tests | `src/expander_properties.rs`: six properties for Unicode literals/overlap, reset/update/backspace/terminators, empty input retention, word boundaries, case collisions, regex suffix captures, and cursor counts. Regression seeds committed in `proptest-regressions/expander_properties.txt`; commands/case counts in the campaign report. |
| Fix reproduced defects | Three matcher failures were minimized before fixes; isolated pack rename and queued-reload failures also failed before fixes. Regression coverage is in the matcher properties and `packs::tests`/`daemon::tests`. |
| Graph and schema agreement, failure states | `tests/authoring_cli.rs::generated` checks echo output/schema acceptance and echo/nested graphs in both declaration orders, with cycles/missing targets/depth limits. Pack rollback tests cover every rename, partial application, rollback failure, and retained backups. Reload tests cover invalid YAML and filesystem read failure. |
| Bounded echo with dependencies and escaping | `src/template.rs` and `expander::render_bounded` enforce depth, evaluation, and byte budgets. CLI fixtures prove globals/local overrides, nested echo, literal braces, forward references, unknown/cyclic references, and failure before stdout. Unit tests cover optional captures and Unicode keymap collection; pack CLI exercises echo imports. Rules and deliberate Espanso differences are in `docs/variables.md`. |
| Named personal groups: configuration and CLI | `snippet_groups`, strict group validation, `group list/enable/disable/toggle`, editor schema, and inventory `groups`/`available` metadata. `tests/groups_cli.rs` exercises actual CLI defaults, memberships, root-relative paths, schema/parser boundaries, and errors. |
| IPC and persistence | `src/groups.rs`: versioned state, exclusive lock, atomic replacement, preserved unknown overrides. Tests cover write/lock failures, restored definitions, malformed state, 12 concurrent toggles, and lost responses without replay. A daemon test crosses the real Unix socket/parser/client/mutation handler and reloads persisted state, without desktop initialization. |
| Defined profile/pack/reference behavior | One `active_match_indices` selection feeds startup, reload, profile changes, preview, and paste. Group and profile filters intersect; nested dependents are suppressed transitively. Group CLI tests verify overlaps, profiles, source-bypass rejection, and active duplicate disambiguation. Pack CLI verifies independent controls and rejection of a disable that would break a static reference. |
| Preserve unfinished Unicode and live setup | Edits were confined to the authoring checkout and handoff documents. Read-only inspection confirms `codex/fcitx5-direct-commit` still has its WIP, the plugin checkout is clean, and the existing installed service remains active. No installation/service-mutating commands were issued. |
| Automated verification | 125 tests: 95 unit, 15 authoring CLI, 8 group CLI, 7 pack CLI. Formatting, Clippy with warnings denied, CLI help, diff checks, and package verification pass. A fresh target directory independently built and tested current source. |
| Documentation/task handoff and local commits | README, schemas guide, compatibility, pack docs, group contract, variable rules, bundled shortcut instructions (2.3.0), TASKS, checkpoints, roadmap, campaign report, and this audit are updated. Changes are committed locally. |

## Commands and practical limits

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
TMPDIR="$PWD/target/test-tmp" cargo test --locked
cargo run --locked -- --help
cargo run --locked -- group --help
CARGO_TARGET_DIR="$PWD/target/package-verification" \
  TMPDIR="$PWD/target/test-tmp" cargo package --locked --allow-dirty
git diff --check
```

The fresh-build check used `CARGO_TARGET_DIR=target/batch-verification`. An older
package verification had polluted the default Cargo cache with an executable
from the unpacked package tree. That generated `target/debug` directory was
moved to recoverable desktop Trash, then ordinary `cargo run` was rebuilt and
verified. Keep package verification in its separate target directory.

Earlier extended campaigns passed 60,000 matcher and 6,000 graph/schema cases
with seed `20261002`; the normal smoke suite also passed on the final code.
Graph smoke and the branching budget test passed under a 2 GiB virtual-memory
cap and a 30-second process deadline. These checks do not prove power-loss
recovery, simultaneous pack-manager correctness, or GUI injection compatibility.
See [the campaign report](testing/generated-suite.md) for exact scope/results.

## Next useful work

1. Review/integrate the authoring branch and reconcile the unfinished Unicode
   branch before choosing a combined release. Do not overwrite that worktree.
2. Add plugin group controls using the completed IPC and inventory metadata.
3. Continue the live compatibility matrix only with authorization for the
   desktop actions described in the Unicode handoff.

None of these later tasks was silently added to this completed goal.
