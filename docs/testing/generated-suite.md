# Generated reliability suite

## Running

Normal `cargo test --locked` runs 256 cases per Proptest property plus saved
regressions. The suite uses disposable files and pure matching/rendering code.
It does not type into the desktop. Keep `proptest-regressions/` under version
control; failed cases are reduced and saved there automatically.

Reproduce the initial campaign (or increase cases for a bounded longer run):

```sh
mkdir -p target/test-tmp
TMPDIR="$PWD/target/test-tmp" PROPTEST_RNG_SEED=20261002 \
  PROPTEST_CASES=10000 timeout 180s \
  cargo test --locked --bin snipexpand expander::properties
```

The seed and Cargo.lock pin the generator inputs. A failure prints the minimized
input and regression hash. Add a specific example test when changing the model
or strategy would otherwise lose a regression. The default smoke suite runs in
CI through its existing `cargo test` step.

## Matcher properties

- Literal suffix selection versus an independent text-history model, including
  duplicate/overlapping Unicode triggers, immediate and terminated modes,
  Space/Tab, backspace, reset, and configuration updates.
- Empty match sets retain no typed characters.
- Whole-word matching checks the preceding character and preserves the trailing
  separator. The buffer retains the longest trigger plus two boundary characters.
- Case-propagating trigger collisions never select an arbitrary replacement.
- Regex suffix captures and Unicode cursor offsets have explicit output checks.

The history model discards characters beyond the documented buffer capacity:
backspace cannot recover input that has already been forgotten. These tests do
not establish desktop injection correctness or model every regex expression.

## Reproduced defects — 2026-10-02

Seed `20261002` reproduced three defects before fixes:

1. Empty matcher + one `a` retained input; continued typing grew the buffer.
2. Triggers `a` and `A`, with case propagation enabled on one, expanded an
   arbitrary entry on `a ` instead of requiring explicit selection.
3. Whole-word trigger `a` expanded the last `a` in `aa `: appending the separator
   discarded the preceding boundary character before checking it.

Fixes return early for an empty matcher, classify case-propagating collisions
when compiling matches, and retain both boundary characters. Literal and regex
collision keys now use distinct namespaces. Explicit preview/paste still selects
an exact configured trigger, with source disambiguation for exact duplicates.

Further graph, schema, rendering-budget, and filesystem-failure coverage belongs
to the current batch; this report will be extended as those slices are verified.

Verified matcher campaign: 10,000 cases × 6 properties, seed `20261002`,
8.05 seconds test runtime, all passed after fixes. Full baseline after this
slice: 95 tests passed; Clippy with warnings denied, formatting, and CLI help
passed. No live setup changes.

## Variable graphs and schemas

Three further properties generate echo DAGs, echo chains, and nested-match
chains. They check exact Unicode output, forward/reverse declaration order,
branching, cycles, missing targets, depth 1–74, and echo schema acceptance
against the real `render` CLI. Static semantic failures may pass the schema;
that distinction is checked explicitly in the selected invalid fixtures.

Campaign: 2,000 cases × 3 properties, seed `20261002`, 11.59 seconds, passed.
Run it with the same command above using `--test authoring_cli generated::`
in place of `--bin snipexpand expander::properties`.
Selected cases also check escaping, globals/local overrides, optional regex
captures, echo in packs, Wayland character collection, cumulative byte limits,
and a branching nested graph that produces empty text but exceeds the evaluation
budget. Limits cover both validation recursion and rendering.

The branching evaluation regression and graph smoke suite also pass under
`ulimit -v 2097152` (2 GiB virtual memory) and a 30-second `timeout` per command.
After echo: 107 tests passed; Clippy with warnings denied, formatting, and CLI
help passed. Filesystem-failure coverage and named groups remain outstanding.

## Filesystem failures and reloads

Controlled rename failures reproduced a pack-update bug: if moving the installed
repository to its backup failed after moving the active mirror, the mirror was
not restored. The transaction now rolls back every completed move after failures
at each rename or partial application. Tests inject a second rollback failure to
verify that originals remain in named recovery paths and a retry refuses to
overwrite them. Existing CLI tests verify successful updates, disabled installs,
and rejection of invalid pack contents. This covers operation failures, not
power-loss durability or simultaneous pack-manager processes.

The daemon reload state transition is now testable without a keyboard/injector.
Invalid YAML and filesystem read failures retain the last valid configuration.
A successful reload cancels buffered input, undo, and queued expansions; a
regression demonstrated that an old queued expansion previously survived.
IPC reload returns errors to the caller, and the CLI reads the response instead
of reporting success immediately. A local mock-socket integration test checks
that rejection is visible to the caller. No real daemon or config was used.

After rollback/reload changes: 111 tests passed (90 unit, 15 authoring CLI,
6 pack CLI), Clippy with warnings denied, formatting, and CLI help passed.
