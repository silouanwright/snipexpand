# AGENTS.md

## Project purpose

`snipexpand` is a Rust-based, Wayland-native text expander for Linux. It reads physical keyboard events through `evdev`, matches configured triggers, and injects expansions via a `uinput` virtual keyboard. Wayland is used only to fetch the active XKB keymap.

## Repository map

- `src/main.rs`: CLI entry point, subcommands, daemon startup, service installer
- `src/daemon.rs`: main async runtime and event loop
- `src/keyboard.rs`: physical keyboard discovery and evdev event ingestion
- `src/injector.rs`: XKB keymap handling and uinput-based text injection
- `src/expander.rs`: rolling buffer and trigger matching
- `src/config.rs`: strict YAML loading, legacy TOML compatibility, and validation
- `src/ipc.rs`: UNIX socket IPC used by `reload` and `status`
- `target/`: generated build artifacts, never edit manually

## Commands

```bash
cargo build
cargo build --release
cargo test
cargo run -- --help
install -Dm755 target/release/snipexpand ~/.local/bin/snipexpand
systemctl --user start snipexpand
systemctl --user stop snipexpand
systemctl --user restart snipexpand
systemctl --user status snipexpand
journalctl --user -u snipexpand -f
```

## Runtime/config paths

- settings: `~/.config/snipexpand/config.yml`
- matches: `~/.config/snipexpand/match/**/*.yml` (`.yaml` also supported)
- legacy config: `~/.config/snipexpand/expansions.toml`
- IPC socket: `$XDG_RUNTIME_DIR/snipexpand.sock`
- user service: `~/.config/systemd/user/snipexpand.service`

## Architecture notes

- Physical keyboard input comes from `/dev/input/event*`.
- The daemon tracks modifier state and decodes keys with the actual XKB keymap.
- `Expander` performs suffix matching against typed match records, including
  word boundaries and configurable Space/Enter/Tab termination.
- Injection sends backspaces first, expanded text, then any Left-arrow events
  required by a `$|$` cursor marker.
- The uinput device name is intentionally `snipexpand virtual keyboard` so it can be filtered out and not re-read as physical input.

## Important constraints / pitfalls

- GNOME does not expose `zwp_virtual_keyboard_manager_v1`; do not reintroduce protocol-based injection unless compositor support is verified.
- `uinput` timing delays matter. Removing them causes modifier bleed and scrambled output.
- The Wayland keymap payload may include a trailing `\0`; trim it before building XKB structures.
- Config watcher threads must not block runtime shutdown.
- Duplicate triggers are rejected with both source paths; longer matches are
  evaluated first so prefix-related triggers remain deterministic.
- Unsupported Espanso fields must fail validation rather than being ignored.
- Avoid editing generated files under `target/`.
- This repo may contain local, not-yet-pushed work. Check `git status --short` before making broader changes.

## Validation workflow

Before finishing code changes:

1. run `cargo test`
2. run `cargo run -- --help`
3. if service/install logic changed, review `src/main.rs` service template carefully
4. inspect `git diff --stat` and `git status --short`

## Done criteria

- changes are limited to intended files
- docs/README stay aligned with the actual CLI and architecture
- no generated artifacts are committed accidentally
- validation commands pass, unless the user explicitly asks to skip them

## GitHub access

Push access requires the `silouanwright` GitHub account. Always switch before pushing:

```bash
gh auth switch --user silouanwright
git push origin <branch>
```

## Git workflow

This is currently a solo project. Verified changes may be committed and pushed
directly to `main`. Use a feature branch or pull request when it materially
helps review a risky or collaborative change, not as a mandatory ceremony.

Before pushing, run the validation workflow above and review the final diff.
CI remains available as an additional check when a pull request is useful.

## Cutting a release

A tag on `main` triggers the full release pipeline. CI builds binaries, creates a GitHub Release, and publishes to crates.io automatically.

```bash
# Manual (no extra tools needed):
# 1. Edit version in Cargo.toml  (patch/minor/major per semver)
# 2. git add Cargo.toml && git commit -m "chore: release vX.Y.Z"
# 3. git tag vX.Y.Z && git push && git push --tags

# Optional shortcut with cargo-release:
cargo release patch   # or minor / major
git push && git push --tags
```

Monitor: `gh run watch` / `gh release list`

**Note:** SnipExpand is a new crate name. Its first crates.io publication requires
a token with permission to publish a new crate; later releases can use a
publish-existing-crate token.
