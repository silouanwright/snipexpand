# SnipExpand tasks

This is the prioritized product backlog. Within each group, work from top to
bottom unless user feedback provides a stronger signal.

## P0: Make the current product dependable

These tasks should come before adding major expansion features.

- [ ] Build a repeatable application compatibility suite.
  - [x] Establish an isolated, byte-exact Neovim-in-Foot baseline using the
    real evdev and Wayland injection path.
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
  - Ensure installation, input-group access, service setup, upgrade, and removal
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
- [ ] Add named snippet groups and quick enable or disable controls.
  - Support global and application-scoped groups.
  - Expose group state through the CLI and status JSON.
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
