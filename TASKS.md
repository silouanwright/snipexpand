# SnipExpand tasks

This is the prioritized product backlog. Within each group, work from top to
bottom unless user feedback provides a stronger signal.

## Graphical release gate and prepared image — 2026-10-23

Status: implemented and validated locally. Compatibility commit `095baaa` is
integrated on `codex/integrated-release` as `5a0494a`; prior VM documentation and
continuity were preserved in `5a5801b`. No production SnipExpand change was needed.

- `python3 tests/vm/run.py` now runs Firefox, Zed, Neovim, Chromium, and native
  QMP expansion. Normal tests verify installed packages without downloading or
  updating them. The shared runner automatically takes the local build budget.
- `tests/vm/release-check.py run --output /outside/checkout/evidence` requires a
  clean commit, all five checks, a sealed image, matching source/binary/harness
  hashes, and complete cleanup. An explicit `publish` posts the receipt hash to
  GitHub. The tag workflow requires the latest trusted `desktop-vm/full` status
  for that exact commit before either architecture build. CI runs the negative
  gate tests. Nothing has been pushed or published in this batch; the remote
  release gate takes effect after these workflow changes are pushed.
- Prepared and activated `desktop-20261023-v1`: original base hashes unchanged,
  two verified OS shutdowns, a reboot into kernel `7.2.8-9-omarchy`, standalone
  qcow2, pinned disk/firmware hashes. Apps: Firefox 158.0.1, Zed 1.25.2, Chromium
  155.0.8059.84, Neovim 0.12.6; Omarchy package 4.0.6, Hyprland 0.56.2.
  This was system-package provisioning, not the full Omarchy migration sequence.
- All five SnipExpand checks passed on that image. LookElsewhere also passed
  guest installation, native Escape dismissal, and live reliability. Its test
  installer needed a bounded wait for asynchronous plugin discovery; the
  test-only companion commit is `8c45c23688be667ef0e13dc9e7422196bc99fcd7`.
  Its preexisting README edit remains untouched. No plugin runtime code changed.
- Passed 149 Rust tests, CLI help, formatting, 15 Python gate/image/routing tests,
  workflow YAML/dependency checks, and an actual CLI rejection of dirty source.
  Original Unicode/Fcitx WIP and dependency commit `91eecb3` remain unchanged.

Evidence: `/home/silouan/Documents/ChatGPT/SnipExpand/verification/20261023-automation/`.
`image-accepted.json` identifies the passing runs for both projects;
`validation.log` records unit checks. The final clean-commit proof belongs in
`release-proof/receipt.json`, produced after this documentation checkpoint by
`release-check.py`; its `commit` must match HEAD before any publication. An absent
receipt means that final step has not completed. This external receipt avoids
changing a tested commit merely to write its own hash into this task record.

The first preparation attempt installed packages but treated the SSH disconnect
on poweroff as an error. Preparation now acknowledges a delayed guest OS shutdown
before waiting for QEMU exit zero. The dedicated provisioning disk was checked,
rebooted, and cleanly stopped twice before sealing; no test overlay was promoted.
The first LookElsewhere image test failed on plugin-discovery timing; its retry
passed after the guest-only readiness fix. Both failures remain in the evidence.

Shared runner code is tracked under `tests/vm/harness/` and deployed through
`tests/vm/install-harness.py`, which backs up code and preserves local state.
[Commands, limits, image refresh and release procedure](tests/vm/README.md).
Remaining delivery: push the verified local commits when requested; publish a
fresh receipt for the actual release commit before pushing its tag. No hosted VM
runner, timer, release, or host desktop changes were created. Broader feature and
keyboard/layout coverage below remains separate future work.

## Firefox/Zed guest compatibility — 2026-10-23

Status: complete locally. Isolated checkout
`/home/silouan/Work/projects/snipexpand-compatibility`, branch
`codex/compatibility-20261023`, commit `095baaa`, based on released integration `84fa42c`.
No production code change or SnipExpand defect was needed for this campaign.

- Firefox 158.0.1 and Zed 1.25.2 both passed the 117-byte fixture: ASCII,
  Unicode/non-BMP emoji, multiline, cursor placement, immediate Backspace undo,
  and twenty consecutive expansions. Each also passed add/change/remove Unicode
  reload probes with the original daemon PID still alive.
- Added the optional `SNIPEXPAND_VM_CAMPAIGN=firefox-zed` adapter and documented
  the command in `tests/vm/README.md`. All graphical work uses guarded disposable
  Oligarchy guests, the shared build limiter, and the VM serialization lock.
- Fixed test setup: Omarchy package-update/install commands with guest sudo,
  Stay Awake during downloads, explicit lock-state input guards, Firefox's clean
  profile, Zed software-GPU allowance and correct custom-profile settings path.
  Zed trust applies only to the disposable synthetic-file profile. SSH stderr
  warnings no longer obscure the result JSON. The local shared runner now routes
  SIGTERM through evidence export/cleanup; its patch is retained with evidence.
- Passed: 149 Rust tests, fmt, CLI help, Clippy with warnings denied, release and
  example builds, Python/Bash syntax checks, mixed-output parsing probe, and the
  final full VM campaign. Every tested payload hash matched the checkout before
  documentation closeout. Oligarchy/QEMU/PostgreSQL ports were closed afterward.

Evidence: [campaign summary](/home/silouan/Documents/ChatGPT/SnipExpand/verification/20261023-compatibility/campaign-summary.json),
[final VM result](/home/silouan/Documents/ChatGPT/SnipExpand/oligarchy-pilot/artifacts/20261023T180451457167Z/snipexpand/result.json),
and [shared-runner patch](/home/silouan/Documents/ChatGPT/SnipExpand/verification/20261023-compatibility/shared-runner.patch).
The run directory contains source/binary hashes, versions, exact text, screenshots,
reload results, Zed settings/logs, and the exported guest archive. Earlier failed
runs remain diagnostic evidence, not compatibility failures or passes.

Remaining campaign work: none. Integrated locally as `5a0494a` in the automation
batch above; nothing was pushed, released, or installed on the host. Original Unicode/Fcitx WIP and dependency
commit `91eecb3` remain separate and unchanged. Limits: one Omarchy/Hyprland guest,
software rendering, existing boot kernel after package upgrades; no physical GPU,
new-kernel reboot, non-US layout, suspend/hotplug, or broad IME coverage. Those
broader keyboard/layout backlog items remain open. The automation batch above supersedes the original network-provisioning
requirement with a prepared, versioned image.

## Dependency maintenance — 2026-10-20

Status: complete locally. Isolated checkout:
`/home/silouan/Work/projects/snipexpand-dependencies`, branch
`codex/dependencies-20261020`, commit `91eecb3137e192026edde8737f542349dc392e75`
(base: integrated `84fa42c`, not the preserved Unicode/Fcitx branch).

Updated tokio 1.53.1 → 1.54.0, serde-saphyr 1.2.0 → 1.3.0,
serde_json 1.0.151 → 1.0.154, clap 4.6.6 → 4.6.7, libc 0.2.189 → 0.2.191,
and proptest 1.11.0 → 1.12.0, plus compatible transitive refreshes. CI and
release Cargo commands now use `--locked`. Existing Actions major tags already
resolve to current releases. Manifest ranges, toolchain policy, and application
source are unchanged. serde-saphyr 2 and jsonschema 0.59 are deferred pending
separate compatibility review; neither is required by a current advisory.

Passed: 149 Rust tests, formatting, Clippy with warnings denied, release build,
CLI help, and CMake build/CTest (one bridge test), all using Rust 1.99.1 through
the shared absolute Synaxis budget wrapper. RustSec audit before/after: zero
vulnerabilities or warnings. Preserved all 40 existing changed files across
both original checkouts apart from this new task section. Prior reliability
work was not duplicated. No push, merge, release, application install, system
package change, live-service action, or host desktop interaction occurred.

[Full decisions, exact versions, commands and evidence](/home/silouan/Work/projects/snipexpand-dependencies/TASKS.md).
Remaining local maintenance: none. Blockers: none. Review/integration and any
release/install are separate; hosted Ubuntu/ARM CI and graphical VM acceptance
were not run in this offline cycle.


## Offline reliability revalidation — 2026-10-20

- [x] Audit the control-tower request against actual history before reimplementing
  the historical October 2 batch. Matcher (`cd406fa`), graph/schema (`1c40794`),
  and pack/reload fixes (`e2a85a8`) already belong to the integrated history.
- [x] Reverify `84fa42c`: 149 tests; 60,000 matcher cases and 6,000 graph/schema
  cases with seed `20261020`; formatting, Clippy with warnings denied, CLI help.
  Extended campaigns ran under 180-second/2-GiB limits in 5.822 s and 8.428 s.
- [x] Correct the stale next-work list in the preserved original checkout's
  `TASKS.md`; verify its other WIP files are unchanged and retain existing
  uncommitted VM/documentation changes here.

Evidence and exact commands: [verification results](/home/silouan/Documents/ChatGPT/SnipExpand/verification/20261020-reliability/results.json)
and [suite scope](docs/testing/generated-suite.md). No new defect was reproduced;
no production code changes, feature work, publishing, desktop input, or new
services were needed. Requested batch complete; no blockers. Broader application
and keyboard/layout work below remains open and must use the isolated VM path.

## v0.5.0 integration and release — 2026-10-02

- [x] Integrate authoring/reliability/groups with the preserved Unicode/Fcitx work.
- [x] Add native plugin group controls, persistence/error handling, and tests.
- [x] Pass local Rust/C++ checks, package verification, and Ubuntu CI.
- [x] Pass byte-exact Chromium, Electron, GTK, Qt, and clean Neovim delivery tests.
- [x] Install the candidate daemon/bridge/plugin and preserve rollback backups.
- [x] Push daemon and plugin changes to main.
- [x] Complete the final Signal allow/suppress smoke test in an unsent draft.
  Both settings delivered the emoji; this composer did not trigger the sensitive
  hint guard. Drafts were cleared and the normal service restored.
- [x] Verify the refreshed installed plugin view and keyboard controls.
- [x] Tag and publish the daemon and plugin, verify both release artifacts and
  crates.io publication, and record final delivery evidence.

Current evidence: [integration report](docs/release-0.5.0.md).
Firefox/Zed guest coverage is complete above; broader keyboard/layout campaigns remain open.

## Snippet authoring batch — 2026-10-01

Implemented on `codex/snippet-authoring` and integrated with Unicode/Fcitx5
on `codex/integrated-release` for v0.5.0. See the
[integration report](docs/release-0.5.0.md) for current validation and delivery.

- [x] Add fallible date rendering and validate formats, offsets, timezone names,
  variable names, and variable-specific parameters before expansion.
- [x] Add read-only `render` with exact source/profile selection and JSON cursor
  metadata, sharing the daemon renderer without desktop interaction.
- [x] Add settings/match editor schemas, `schema` export, setup instructions,
  and structural/semantic validation fixtures checked against the real CLI.
- [x] Add explicit IANA date timezones, with fixed-clock tests for UTC, calendar
  boundaries, both DST transitions, and positive/negative elapsed offsets.
- [x] Add bounded echo support in the next batch.
- [x] Finish named personal groups in the next batch.
- [ ] Validate live application delivery as part of the existing P0 matrix.

## Completed batch — 2026-10-02

See [the next-work plan](docs/next-work-2026-10-02.md) for scope and acceptance.
The authoring batch is committed as `fc90d52` and included in the v0.5.0 integration.
Completion audit and local commits: [completed authoring batch](docs/completion-authoring-batch-2026-10-02.md).

- [x] Build generated matcher tests with an independent reference model; fix
  reproduced defects and retain minimal regression cases.
- [x] Extend coverage to nested-variable graphs, schema/parser agreement, and
  failed pack/reload operations under isolated filesystem tests.
- [x] Add bounded echo variables with explicit dependency and escaping semantics.
- [x] Add named personal groups with persistent CLI/IPC controls and defined
  profile/pack/reference behavior.

This completed batch used extra compute to explore new cases without live desktop
input. Existing application compatibility work remains open. The docs index
is [docs/README.md](docs/README.md).

## P0: Make the current product dependable

These tasks should come before adding major expansion features.

- [ ] Build a repeatable application compatibility suite.
  - Current state: [v0.5.0 integration report](docs/release-0.5.0.md).
    The [2026-08-30 checkpoint](docs/checkpoint-compatibility-2026-08-30.md) is historical.
  - [x] Establish an isolated, byte-exact Neovim-in-Foot baseline using the
    real evdev and Wayland injection path.
  - [x] Add reusable browser/Electron and Qt targets plus a byte-exact clipboard
    verifier for applications that cannot save the target buffer directly.
  - [ ] Finish application-aware non-BMP input validation. Chromium and Electron
    use the optional Fcitx5 direct-commit bridge with a paced compose fallback;
    terminal applications retain direct modifier-free Wayland keymaps. Current
    Chromium/Electron byte-exact tests pass. Signal delivery passed under both allow/suppress
    settings; its composer did not exercise sensitive-hint blocking. Sequence, modifier cleanup, retry policy,
    and timing configuration have deterministic coverage.
    - [x] Add an explicit input-method-v2 mode for direct UTF-8 commit when the
      focused client supports text-input-v3 and no other input method owns the
      seat. Keep it out of automatic routing because Omarchy's Fcitx5 already
      owns that exclusive protocol role.
  - Cover Chromium, Firefox, Zed, terminals, Electron apps, GTK, and Qt.
  - Exercise ASCII, Unicode, multiline text, cursor placement, undo, rapid
    consecutive expansions, and configuration reloads.
  - Record known application-specific timing overrides as fixtures.
- [x] Make upgrades and removal first-class.
  - Add `snipexpand uninstall` for the user service and generated service file.
  - Ensure `snipexpand install` replaces and restarts an older running daemon.
  - Document which user configuration remains after uninstalling.
- [ ] Offer an Arch-native installation path.
  - Publish and maintain an AUR package for release binaries.
  - Ensure installation, keyboard-only udev access, service setup, upgrade, and removal
    behave naturally on Omarchy and Arch Linux.
  - Keep Cargo and direct binary installation available.
- [x] Rebuild the persistent Unicode keymaps after a configuration reload.
  - Newly added Unicode snippets should not require a daemon restart or
    temporarily fall back to `wtype`.
  - Add regression coverage for adding and removing Unicode while running.
- [ ] Expand keyboard and input testing.
  - [x] Track modifiers independently across multiple keyboards and clear them
    when a device disconnects.
  - Test common non-US XKB layouts, held modifiers, dead keys, compose keys,
    key repeat, multiple keyboards, sleep and resume, and device hotplugging.
  - Document IME and Fcitx5 behavior rather than guessing at compatibility.
- [x] Remove the GitHub Actions Node 20 deprecation warnings.

## P1: Highest-value user features

Completed items document shipped work. Remaining items are candidates for
future feature releases.

- [x] Round out snippet metadata and composition.
  - Support Espanso-compatible `search_terms` and expose them through
    `snipexpand list --json` for the Omarchy plugin.
  - Support safe nested `match` variables so shared text can be reused without
    shell execution or copy-and-paste duplication.
  - Reject missing references and dependency cycles during `snipexpand check`.
- [x] Add pause and resume controls.
  - Expose `enable`, `disable`, and `toggle` over the existing IPC connection.
  - Include the enabled state in `status --json` and the Omarchy plugin.
  - Consider a configurable double-tap modifier only after CLI and plugin
    controls prove insufficient.
- [x] Add per-application profiles.
  - Allow different match sets, trigger modes, and injection timing by title,
    class, or executable.
  - Preserve the current exclusion syntax as the simple path.
  - Use `snipexpand detect` to expose the title, class, and executable values
    needed to write profile filters.
- [x] Add regex triggers with capture variables.
  - Bound the input buffer and regex execution so matching stays predictable.
  - Define deletion, boundaries, case propagation, and overlapping matches
    before accepting the syntax.
- [x] Add a snippet search palette through the Omarchy plugin.
  - Search labels, triggers, and replacement previews.
  - Insert through `snipexpand paste` while keeping the daemon independent of
    the plugin.
- [x] Make word-boundary behavior configurable.
  - Add Espanso-compatible `word_separators` for punctuation, programming, and
    language-specific workflows.
  - Preserve the current Unicode-aware default when the setting is absent.
- [x] Handle duplicate triggers deliberately.
  - Automatic typing expands only when app profiles leave one match active.
  - Let launchers select an exact duplicate with its source path through
    `snipexpand paste --source`.
- [x] Add named personal snippet groups and quick enable or disable controls.
  - Persistent CLI/IPC choices and native Omarchy plugin controls.
  - File/subtree membership intersects application-profile filtering.
  - Structured state is available through `group list --json`; existing status
    `match_groups` retains its original meaning (match definitions).
- [x] Add Git-published snippet packs.
  - Treat each installed pack as a read-only, independently enableable group.
  - Auto-detect Espanso `_manifest.yml` plus `package.yml` repositories and
    install them directly when every used field is supported by SnipExpand.
  - Reject incompatible Espanso packs with a complete field-level report;
    never silently omit snippets, variables, scripts, forms, or settings.
  - Accept public or private Git URLs, an optional repository subdirectory,
    and an optional tag or commit.
  - Provide install, inspect, list, update, and remove commands.
  - Validate every file and report trigger conflicts before enabling a pack.
  - Keep updates explicit and record the installed commit for reproducibility.
  - Let authors use ordinary repositories; do not require a central registry.
  - Install compatible official Hub packages with `espanso:NAME`.
  - Manage installed packs from the Omarchy plugin.
- [ ] Add an optional long-text injection strategy.
  - Benchmark persistent typing before choosing a clipboard or compositor-native
    backend.
  - If clipboard insertion is added, make it explicit, preserve prior clipboard
    contents, and retain the clipboard-free default.

## P2: Differentiators and power-user features

- [ ] Build an opt-in snippet opportunity advisor.
  - Detect only exact manual occurrences of eligible, already-configured
    replacement text. Do not discover arbitrary phrases.
  - Show the highest-value missed opportunity and estimated daily, weekly,
    monthly, and yearly savings.
  - Never persist typed text, hashes, application identities, or event
    timestamps. Store only opaque snippet IDs, aggregate counters, and one
    observation start date.
  - Default off, disclose that field-level password detection is unavailable,
    and provide clear inspect, reset, disable, and deletion controls.
  - Remove decoded-character debug logging before introducing advisor state.
- [ ] Add safe variable types.
  - Consider UUID, random selection, environment values, and clipboard contents
    individually after nested matches.
  - Require explicit opt-in for sensitive sources such as the clipboard.
  - Continue rejecting arbitrary shell and script execution by default.
- [ ] Support multiple cursor stops.
  - Let Tab advance through declared positions after expansion.
  - Define cancellation behavior for mouse input, focus changes, and manual
    cursor movement.
- [x] Ship an Omarchy companion plugin.
  - Search, insert, add, edit, remove, diagnose, and restart snippets and the
    service while keeping the daemon independent of the UI.
  - Verify the released plugin through Omarchy 4.0.0's clean add, validate,
    enable, and placement workflow under an isolated HOME.
- [ ] Explore a cross-platform desktop companion only when another supported
  platform needs one.

## P3: Broader platform work

- [ ] Add an Espanso migration command only after the intended compatibility
  surface is complete.
  - This concerns importing a user's personal Espanso configuration. Compatible
    published Espanso packs may be installed earlier through the pack system.
  - Importing earlier would lock migration behavior to an unfinished feature
    target and create repeated migration churn.
  - Import the supported subset without rewriting the original configuration.
  - Report every skipped or incompatible field with its file and match.
  - Make the result pass `snipexpand check` before writing it.
- [ ] Add tested support for more Wayland compositors, starting with those that
  expose reliable active-window and virtual-keyboard protocols.
- [ ] Investigate macOS and Windows backends without weakening the Linux and
  Wayland implementation.
- [ ] Consider optional encrypted sync only after local import, export, and
  conflict behavior are solid.

## Not planned for the core daemon

These features add substantial security or product complexity and should not be
accepted without a new design decision:

- Arbitrary shell or script execution
- Forms and choice windows inside the daemon
- HTML, images, and application-specific rich-text insertion
- A hosted public package registry
- Mandatory accounts, telemetry, or cloud services

## Prioritization signals

The ordering favors reliability and small additions that strengthen ordinary
text expansion. Espanso's `search_terms`, nested matches, pause controls, and
configurable word separators fit SnipExpand's existing architecture. Forms,
rich text, clipboard automation, and arbitrary code execution do not.

Useful upstream signals:

- [Espanso feature requests and ideas](https://github.com/espanso/espanso/discussions/categories/feature-requests-and-ideas)
- [Wayland application detection and per-app configuration](https://github.com/espanso/espanso/issues/2730)
- [Custom configuration locations for dotfile workflows](https://github.com/espanso/espanso/issues/2382)
- [Wayland expansion reliability](https://github.com/espanso/espanso/issues/1966)

Revisit priorities after the project receives its first substantive user issues
or discussions. Direct SnipExpand feedback should outweigh inferred demand from
other projects.
