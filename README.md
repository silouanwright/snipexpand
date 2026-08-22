# SnipExpand

SnipExpand is a fast, config-based text expander for Linux and Wayland. It was
built for a reliable, native experience on [Omarchy](https://omarchy.org) and
Hyprland.

Type a short trigger such as `;mail` and SnipExpand replaces it with the text
you configured. Expansions work across applications without modifying the
clipboard or requiring editor-specific plugins.

[![CI](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml)
[![Release](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)

## Features

- Immediate or terminator-based expansion
- Plain text and multiline replacements
- Cursor placement with `$|$`
- Recursive YAML configuration with automatic reload
- Multiple triggers for one replacement
- Word boundaries, case propagation, and date variables
- Immediate Backspace undo for simple expansions
- Application exclusions
- Persistent Wayland injection with a `uinput` fallback
- Strict validation and diagnostics

SnipExpand focuses on dependable text expansion. It does not currently run
scripts, display forms, insert rich content, or provide a package registry.
See the [compatibility matrix](docs/compatibility.md) for exact details.

## Alternatives

| Project | Best fit | What SnipExpand provides differently |
| --- | --- | --- |
| [Espanso](https://espanso.org) | Cross-platform automation, forms, scripts, and packages | A smaller surface focused on reliable Wayland input and injection |
| [Texpand](https://github.com/Ghishadow/texpand) | Lightweight Wayland text expansion | A native Rust implementation, strict YAML validation, application exclusions, diagnostics, and a managed user service |
| [SKRT](https://github.com/aaaorg/skrt) | Minimal Rust-based expansion | Espanso-style YAML, multiline matches, cursor placement, automatic reload, and broader runtime tooling |
| SnipExpand | Omarchy, Hyprland, and config-based Wayland expansion | Persistent Wayland injection, a tested `uinput` fallback, and first-class Omarchy defaults |

## Requirements

- Linux with a Wayland session
- Read access to `/dev/input/event*`
- `libxkbcommon` and Wayland client libraries
- Membership in the system `input` group, or equivalent permissions
- `wtype` for the Unicode fallback path

Hyprland is the supported and tested compositor. Other Wayland compositors may
work but are not yet part of the supported test matrix.

## Install

Download the latest x86_64 release:

```bash
mkdir -p ~/.local/bin
curl -L https://github.com/silouanwright/snipexpand/releases/latest/download/snipexpand-x86_64-linux \
  -o ~/.local/bin/snipexpand
chmod +x ~/.local/bin/snipexpand
```

Each [GitHub Release](https://github.com/silouanwright/snipexpand/releases)
also includes an aarch64 binary and SHA-256 checksums.

You can also install with Cargo:

```bash
cargo install snipexpand
```

Building from source requires `libxkbcommon-dev` and `libwayland-dev` on
Debian or Ubuntu, or `libxkbcommon` and `wayland` on Arch Linux.

## Set up

```bash
snipexpand init
sudo usermod -a -G input "$USER"
```

Log out and back in so the group change reaches your graphical session. Then
install and start the systemd user service:

```bash
snipexpand install
snipexpand doctor
```

The service starts immediately and is enabled for future graphical sessions.

## Configure

Add a simple expansion from the command line:

```bash
snipexpand add ';mail' 'user@example.com'
```

For advanced matches, create or edit a YAML file below
`~/.config/snipexpand/match/`:

```yaml
global_vars:
  - name: today
    type: date
    params:
      format: "%Y-%m-%d"

matches:
  - triggers: [";mail", ";email"]
    replace: "user@example.com"

  - trigger: ";sig"
    word: true
    replace: |
      Best regards,
      Your Name

  - trigger: ";function"
    replace: |
      fn example() {
          $|$
      }

  - trigger: ";today"
    replace: "{{today}}"
```

The first `$|$` marker is removed and the cursor is placed at that position.
Match files reload automatically when saved.

Settings live in `~/.config/snipexpand/config.yml`:

```yaml
trigger_mode: space
terminators: [space, enter]
injection_backend: auto
injection_delay_ms: 1
wayland_injection_delay_ms: 0
uinput_injection_delay_ms: 1
injection_settle_ms: 10
undo_enabled: true
app_exclusions:
  - class: "^1Password$"
  - class: "^org\\.keepassxc\\.KeePassXC$"
```

`trigger_mode` can be `immediate` or `space`. Space mode waits for a
configured Space, Enter, or Tab terminator. Immediate mode expands as soon as
the trigger is complete.

`injection_backend: auto` prefers persistent Wayland injection and uses
`uinput` when the compositor does not expose the required protocol. Adjust
the timing values if a particular application drops or reorders keystrokes.

Run `snipexpand detect` while an application is focused to find the title,
class, and executable values needed for an exclusion.

## Commands

```text
snipexpand                       Run the daemon in the foreground
snipexpand init                  Create starter configuration
snipexpand add TRIGGER TEXT      Add or replace a generated expansion
snipexpand remove TRIGGER        Remove a generated expansion
snipexpand list                  List triggers and source files
snipexpand check                 Validate configuration
snipexpand detect                Inspect the focused application
snipexpand reload                Reload the running daemon
snipexpand status                Show daemon and configuration status
snipexpand status --json         Emit status as JSON
snipexpand doctor                Diagnose setup and runtime requirements
snipexpand install               Install and start the user service
```

Useful service commands:

```bash
systemctl --user status snipexpand
journalctl --user -u snipexpand -f
```

## Undo

Press Backspace immediately after a plain, single-line expansion to restore its
trigger. Multiline and cursor-positioned expansions do not arm undo because the
cursor state is ambiguous.

## Security

SnipExpand reads physical keyboard events through Linux input devices. A
process with this access can observe keyboard input across applications,
including sensitive text. Install only binaries you trust.

Application exclusions prevent expansion in matching applications, but they do
not stop the daemon from receiving keyboard events. SnipExpand cannot determine
whether a browser currently has a password field focused.

SnipExpand does not execute commands from match files and does not read or
modify the clipboard.

## Documentation

- [Espanso compatibility](docs/compatibility.md)
- [Legacy TOML migration](docs/migration.md)
- [Roadmap](docs/espanso-roadmap.md)

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

[MIT](LICENSE)
