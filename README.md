# SnipExpand

Fast, config-based text expansion for Linux and Wayland. **First-class support for [Omarchy](https://omarchy.org) and [Hyprland](https://hypr.land).**

[![CI](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml)
[![Release](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml)
[![License: GPL v3+](https://img.shields.io/badge/license-GPLv3%2B-blue.svg)](LICENSE)

<p>
  <a href="docs/assets/hero-demo.mp4">
    <img src="docs/assets/hero-demo.gif" width="640" alt="SnipExpand replacing short triggers with an email address, emoji, Unicode text, a multiline signature, and code">
  </a>
</p>

## Features

- System-wide, clipboard-free expansion
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

## Why SnipExpand?

Text expansion on Omarchy and Hyprland is still unreliable or awkward. Compare
SnipExpand with the alternatives below.

## Alternatives

| Project | Strengths | Why choose SnipExpand instead |
| --- | --- | --- |
| [Espanso](https://espanso.org) | Cross-platform automation, forms, scripts, and packages | Built for Omarchy and Hyprland; Espanso has documented Linux and Wayland issues with [application compatibility](https://github.com/espanso/espanso/issues/2162), [startup reliability](https://github.com/espanso/espanso/issues/2223), and [expansions stopping over time](https://github.com/espanso/espanso/issues/2423) |
| [Taurine](https://github.com/ereinaimer/taurine) | Cross-platform Rust automation with scripts, conversions, and optional AI | A local-only core, YAML configuration, persistent Wayland injection, and GPL licensing |
| [FlitKey](https://github.com/swarajnandedkar/FlitKey) | A graphical picker with hotkeys, imports, and expansion packs | Typed Wayland expansion instead of copy and paste, with no Python GUI runtime |
| [AutoKey for Wayland](https://github.com/dlk3/autokey-wayland) | GUI automation and Python scripting | Hyprland support and a native Rust daemon; AutoKey's Wayland fork targets GNOME |
| [Texpand](https://github.com/andresousadotpt/texpand) | Lightweight Go, YAML, and cursor placement | Rust, persistent Wayland injection, validation, exclusions, and diagnostics |
| [text-expander-wayland](https://github.com/quantavil/text-expander-wayland) | Rust, Espanso-style YAML, variables, and optional AI | Persistent injection instead of launching `wtype` or `ydotool` for each expansion |
| [SRKT](https://github.com/aaaorg/srkt) | A small Rust foundation for Wayland expansion | YAML, multiline matches, cursor placement, reloads, exclusions, and runtime tooling |

## Requirements

Omarchy includes everything SnipExpand needs by default. On other Wayland
systems, you need:

- `libxkbcommon` and Wayland client libraries
- Read access to `/dev/input/event*`, usually through the system `input` group
- `wtype` for the Unicode fallback path

## Install

Install from crates.io:

```bash
cargo install snipexpand
```

Prebuilt x86_64 and aarch64 binaries are available from
[GitHub Releases](https://github.com/silouanwright/snipexpand/releases).

## Set up

```bash
snipexpand install
```

This creates any missing starter files without overwriting your config, then
starts the service and enables it for future sessions. Run `snipexpand doctor`
if setup or expansion does not work.

## AI agents

SnipExpand installs an AI skill at `~/.config/snipexpand/SKILL.md`. Point your
coding agent at it and describe what you want:

> Read `~/.config/snipexpand/SKILL.md`, then set up my SnipExpand snippets.

The skill teaches agents how to scaffold and edit match files, change settings,
use the CLI, validate changes, and check the running service. You can manage
your entire setup this way without learning the commands below.

## Add your first snippet

Add an expansion from the command line:

```bash
snipexpand add ';mail' 'user@example.com'
```

## Match files

For more control, create or edit a YAML file below
`~/.config/snipexpand/match/`. SnipExpand offers
[best-effort compatibility](docs/compatibility.md) with Espanso's YAML match
format:

```yaml
# Match files reload when saved.
global_vars:
  - name: today
    type: date
    params:
      format: "%Y-%m-%d"

matches:
  # Multiple triggers, one replacement
  - triggers: [";mail", ";email"]
    replace: "user@example.com"

  # Whole-word matching and multiline text
  - trigger: ";sig"
    word: true
    replace: |
      Best regards,
      Your Name

  # $|$ marks the cursor position after expansion.
  - trigger: ";function"
    replace: |
      fn example() {
          $|$
      }

  # Insert a formatted date
  - trigger: ";today"
    replace: "{{today}}"
```

## Settings

Edit `~/.config/snipexpand/config.yml`:

```yaml
# Choose when expansion happens.
trigger_mode: space        # immediate | space
terminators: [space]       # any of: space, enter, tab

# Prefer native Wayland injection and fall back to uinput.
injection_backend: auto    # auto | wayland | uinput

# Tune these only if an application drops or reorders characters.
injection_delay_ms: 1
wayland_injection_delay_ms: 0
uinput_injection_delay_ms: 1
injection_settle_ms: 10

# Backspace immediately after a simple expansion to restore its trigger.
undo_enabled: true         # true | false

# Optional. Disable expansion in matching applications. Default: []
app_exclusions:
  - class: "^1Password$"
  - class: "^org\\.keepassxc\\.KeePassXC$"
```

Run `snipexpand detect` while an application is focused to find the title,
class, and executable values needed for an exclusion.

## Commands

```text
snipexpand [COMMAND]
sxp [COMMAND]                    Short form

(no command)                     Run the daemon in the foreground
init                             Explicitly create starter configuration
add TRIGGER TEXT                 Add or replace a generated expansion
remove TRIGGER                   Remove a generated expansion
list                             List triggers and source files
check                            Validate configuration
detect                           Inspect the focused application
reload                           Reload the running daemon
status [--json]                  Show daemon and configuration status
doctor                           Diagnose setup and runtime requirements
install                          Install and start the user service
uninstall                        Remove the service; preserve configuration
```

## Limitations

- Hyprland is the only supported and tested compositor. Other Wayland
  compositors may work but are not yet part of the test matrix.
- Undo works only immediately after a plain, single-line expansion. Multiline
  and cursor-positioned expansions cannot be undone back to their trigger.
- SnipExpand does not run scripts or shell commands, display forms, insert rich
  text or images, or provide a package registry.
- Matches are literal triggers rather than regular expressions. Variables are
  currently limited to formatted dates.
- Application exclusions operate at the application level. Wayland does not
  expose a browser's focused field type, so SnipExpand cannot automatically
  identify password fields inside an allowed browser.
- SnipExpand reads global keyboard events, including sensitive input.
  Application exclusions stop expansion but do not stop the daemon from
  receiving those events. Install only binaries you trust.

SnipExpand does not execute snippets, access the clipboard, or contact online
services.

See the [compatibility matrix](docs/compatibility.md) for the complete supported
configuration surface.

## Documentation

- [Prioritized tasks](TASKS.md)
- [Espanso compatibility](docs/compatibility.md)
- [Espanso-informed design notes](docs/espanso-roadmap.md)

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

[GNU General Public License v3.0 or later](LICENSE).

Other platforms offer polished text expansion built in or through expensive
software. Linux users should not have to settle for less or pay a costly
subscription for basic infrastructure. SnipExpand is free and open source so
anyone can use it, study it, improve it, and share it.

Anyone who distributes a modified version must make its source available under
compatible terms. The project cannot be repackaged and distributed as
closed-source software. Private use and private modifications remain private.
