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
  - Test common non-US XKB layouts, held modifiers, dead keys, compose keys,
    key repeat, multiple keyboards, sleep and resume, and device hotplugging.
  - Document IME and Fcitx5 behavior rather than guessing at compatibility.
- [ ] Remove the GitHub Actions Node 20 deprecation warnings.

## P1: Highest-value user features

These are the strongest candidates for the next feature releases.

- [ ] Add per-application profiles.
  - Allow different match sets, trigger modes, and injection timing by title,
    class, or executable.
  - Preserve the current exclusion syntax as the simple path.
  - Make `snipexpand detect` generate or suggest the relevant YAML.
- [ ] Add an Espanso migration command.
  - Import the supported subset without rewriting the original configuration.
  - Report every skipped or incompatible field with its file and match.
  - Make the result pass `snipexpand check` before writing it.
- [ ] Add regex triggers with capture variables.
  - Bound the input buffer and regex execution so matching stays predictable.
  - Define deletion, boundaries, case propagation, and overlapping matches
    before accepting the syntax.
- [ ] Add a snippet search palette.
  - Provide fuzzy search by trigger, replacement preview, and optional label.
  - Keep the daemon usable without any GUI process.
  - Avoid stealing focus or breaking insertion into the original application.
- [ ] Add named snippet groups and quick enable or disable controls.
  - Support global and application-scoped groups.
  - Expose group state through the CLI and status JSON.
- [ ] Add an optional long-text injection strategy.
  - Benchmark persistent typing before choosing a clipboard or compositor-native
    backend.
  - If clipboard insertion is added, make it explicit, preserve prior clipboard
    contents, and retain the clipboard-free default.

## P2: Differentiators and power-user features

- [ ] Build an opt-in snippet opportunity advisor.
  - Detect text the user repeatedly types even though an equivalent snippet
    already exists.
  - Show the highest-value missed opportunity and estimated daily, weekly,
    monthly, and yearly savings.
  - Keep captured text local, make retention configurable, and provide a clear
    way to inspect and delete all collected data.
- [ ] Add safe variable types.
  - Consider UUID, random choice, environment values, and clipboard contents
    individually.
  - Require explicit opt-in for sensitive sources such as the clipboard.
  - Continue rejecting arbitrary shell and script execution by default.
- [ ] Support multiple cursor stops.
  - Let Tab advance through declared positions after expansion.
  - Define cancellation behavior for mouse input, focus changes, and manual
    cursor movement.
- [ ] Add Git-based snippet sharing.
  - Start with import and export of ordinary directories.
  - Pin revisions and validate all imported YAML.
  - Do not build a hosted package registry until this proves insufficient.
- [ ] Explore a small desktop companion.
  - Prioritize status, pause, search, editing, diagnostics, and update guidance.
  - Keep configuration as readable YAML and the daemon independent of the GUI.

## P3: Broader platform work

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

The ordering favors reliability, installation, application-aware behavior,
migration, regex matching, and discoverability. Those needs recur in mature
text-expander communities, while SnipExpand's strongest advantage remains a
small, reliable Wayland-native core.

Useful upstream signals:

- [Espanso feature requests and ideas](https://github.com/espanso/espanso/discussions/categories/feature-requests-and-ideas)
- [Wayland application detection and per-app configuration](https://github.com/espanso/espanso/issues/2730)
- [Custom configuration locations for dotfile workflows](https://github.com/espanso/espanso/issues/2382)
- [Wayland expansion reliability](https://github.com/espanso/espanso/issues/1966)

Revisit priorities after the project receives its first substantive user issues
or discussions. Direct SnipExpand feedback should outweigh inferred demand from
other projects.
