<div align="center">
  <img src="icon.svg" width="150" alt="SnipExpand balloon logo">

  # SnipExpand

  **Short triggers. Complete text.**

  [![CI](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/ci.yml)
  [![Release](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml/badge.svg)](https://github.com/silouanwright/snipexpand/actions/workflows/release.yml)
  [![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)
</div>

SnipExpand is a focused text expander for Linux/Wayland, developed specifically to
work flawlessly on [Omarchy](https://omarchy.org). It listens for shortcuts
through `evdev` and types their replacements through a `uinput` virtual
keyboard, avoiding application-specific integrations and clipboard mutation.

It is built and tested primarily on Omarchy and Hyprland. SnipExpand is young and
its first release is deliberately focused, but the ambition is larger: make text
expansion best-in-class again, beginning with the Linux/Wayland environments
where reliability remains difficult. Cross-platform backends may follow without
changing the config-first product at its core.

## Why SnipExpand?

Espanso became the gold standard for config-based text expansion for good
reason. It made shortcuts portable, inspectable, versionable, and independent
of any one editor or application. It also grew into a capable cross-platform
automation system with scripts, forms, packages, rich content, and a mature
community.

SnipExpand builds on that model, a personal shorthand library owned as ordinary
files, while starting with reliable Linux/Wayland input. Its kernel-level input
path works without waiting for every application to expose an integration. The
first release keeps a tight scope so the foundation can be understood,
validated, and trusted; that scope is a starting point, not the ceiling.

AI makes that kind of shorthand more useful, not less. Generative tools are
excellent when the answer needs thought or variation. Text expansion is better
when the answer is already known: an email address, a signature, a command, a
template, a link, or a phrase you type every day. SnipExpand is instant,
deterministic, local, available in every text field, and easy for either you or
an AI coding agent to maintain as plain YAML.

### Why start with Linux and Wayland?

Wayland intentionally limits global input and injection, and dependable text
expansion still falls between the compositor, keyboard layout, permissions, and
application toolkit. This is not only a theoretical concern. Espanso users have
reported the service appearing active while expansions stop until a manual
restart ([#2223](https://github.com/espanso/espanso/issues/2223)), expansion
stopping after hours of use ([#2423](https://github.com/espanso/espanso/issues/2423)),
input detection silently ending on Arch/KDE Wayland
([#2262](https://github.com/espanso/espanso/issues/2262)), application-specific
failures ([#2162](https://github.com/espanso/espanso/issues/2162)), and keyboard
layout or Unicode problems ([#1868](https://github.com/espanso/espanso/issues/1868),
[#2497](https://github.com/espanso/espanso/issues/2497)). These reports cover
different systems and do not imply that Espanso fails for everyone. They do
show why Linux/Wayland deserves a focused reliability effort of its own.

Omarchy is SnipExpand's first proving ground: an opinionated Arch/Hyprland system
where the whole path can be tested as one experience. The architecture is not
tied to Omarchy, and future platform backends remain possible.

## SnipExpand and Espanso

Espanso remains the benchmark for breadth and ecosystem. SnipExpand is competing
first on reliability, immediacy, configuration quality, and a polished
Linux/Wayland experience, then growing from that foundation.

| Capability | SnipExpand | Espanso |
| --- | --- | --- |
| Primary focus | Best-in-class expansion, starting on Linux/Wayland | Cross-platform expansion and automation |
| Tested desktop | Hyprland | Linux, macOS, and Windows environments |
| Configuration | Espanso-style YAML | YAML |
| Static and multiline matches | Yes | Yes |
| Cursor placement | `$|$` | `$|$` |
| Word boundaries | Yes | Yes |
| Case propagation | Yes | Yes |
| Date variables | Yes | Yes |
| Backspace undo | Simple expansions | Yes |
| Application exclusions | Title/class/executable regex | Per-app configuration and filters |
| Unicode outside the active layout | `wtype` fallback | Platform-dependent backends |
| Shell/scripts and dynamic variables | No | Yes |
| Forms, choices, images, and rich text | No | Yes |
| Packages and community ecosystem | No | Yes |
| Clipboard injection backend | No | Yes |
| Configuration validation | Rejects unknown fields | Broader schema |
| Maturity | Early release | Established project |

See the exact [compatibility matrix](docs/compatibility.md) before reusing an
existing Espanso configuration.

## Highlights

- Immediate or terminator-based expansion
- Recursive YAML match files with automatic reload
- Multiple triggers for one replacement
- Multiline replacements and `$|$` cursor placement
- Word boundaries and case propagation
- Date variables with formatting and offsets
- Unicode fallback through `wtype`
- Immediate Backspace undo for simple expansions
- Regex-based application exclusions
- Strict configuration validation and runtime diagnostics
- A small CLI for managing generated shortcuts

## Platform support

SnipExpand currently supports Linux/Wayland and is tested on Hyprland. Basic text
expansion may work on other compositors, but they are not yet in the supported
test matrix. Active-application detection uses `hyprctl` on Hyprland and falls
back to `wlrctl` where available.

Requirements:

- Linux with readable `/dev/input/event*` devices and writable `/dev/uinput`
- A Wayland session
- `libxkbcommon` and Wayland client libraries
- `wtype` for characters absent from the active keyboard layout and for undo
- Membership in the system `input` group, unless equivalent device permissions
  are configured

## Security model

SnipExpand reads physical keyboard events at the kernel input layer. Membership in
the `input` group therefore allows SnipExpand, and any other process running as your
user that opens those devices, to observe keyboard input across applications,
including sensitive text. Install only binaries you trust.

SnipExpand cannot determine whether a browser currently focuses a password field.
Use `app_exclusions` for password managers and other sensitive applications.
Exclusions prevent expansion; they do not prevent the process from receiving
the underlying keyboard events.

SnipExpand does not execute shell commands from match files and does not use or
modify the clipboard. Its optional `wtype` fallback creates input through the
compositor when a character cannot be produced by the active XKB layout.

## Install

### Prebuilt binary

Download the binary for your architecture from
[GitHub Releases](https://github.com/silouanwright/snipexpand/releases):

```bash
# x86_64
curl -L https://github.com/silouanwright/snipexpand/releases/latest/download/snipexpand-x86_64-linux \
  -o ~/.local/bin/snipexpand
chmod +x ~/.local/bin/snipexpand
```

An `aarch64` binary and SHA-256 checksum files are attached to each release.

### Cargo

```bash
cargo install snipexpand
```

Building from source requires `libxkbcommon-dev` and `libwayland-dev` on
Debian/Ubuntu, or `libxkbcommon` and `wayland` on Arch Linux.

## Set up

Create a starter configuration:

```bash
snipexpand init
```

Grant access to the physical input devices, then log out and back in so the new
group membership reaches your graphical session:

```bash
sudo usermod -a -G input "$USER"
```

After logging back in, install and start the systemd user service:

```bash
snipexpand install
snipexpand doctor
```

For logs and service state:

```bash
snipexpand status
systemctl --user status snipexpand
journalctl --user -u snipexpand -f
```

## Configure

SnipExpand keeps settings separate from match files:

```text
~/.config/snipexpand/
├── config.yml
└── match/
    ├── personal.yml
    ├── coding.yml
    └── generated.yml
```

All `.yml` and `.yaml` files below `match/` are loaded recursively and watched
for changes. `snipexpand add` and `snipexpand remove` touch only `generated.yml`, so
they never reformat handwritten match files.

### Settings

```yaml
# ~/.config/snipexpand/config.yml
trigger_mode: space
terminators: [space, enter]
injection_delay_ms: 2
injection_settle_ms: 10
undo_enabled: true
app_exclusions:
  - class: "^1Password$"
  - class: "^org\\.keepassxc\\.KeePassXC$"
```

`trigger_mode` can be `immediate` or `space`. In `space` mode, any configured
`space`, `enter`, or `tab` terminator completes a match; SnipExpand removes the
terminator along with the trigger.

`injection_delay_ms` controls the pause after each synthetic key release. The
2 ms default feels effectively immediate on Hyprland while preserving event
ordering. Raise it if an application drops or reorders characters. A value of
`0` is fastest but is not reliable everywhere.

`injection_settle_ms` is a one-time pause before SnipExpand deletes the trigger.
It gives the focused application time to receive the physical keystrokes and
does not pace the replacement itself.

Each application exclusion accepts `title`, `class`, and/or `exec` regular
expressions. Fields within one entry must all match; separate entries are
alternatives. Focus an application and run `snipexpand detect` to discover its
properties.

### Matches

```yaml
# ~/.config/snipexpand/match/personal.yml
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
      Silouan

  - trigger: ";function"
    replace: |
      fn example() {
          $|$
      }

  - trigger: ";today"
    replace: "{{today}}"

  - trigger: ";hello"
    propagate_case: true
    uppercase_style: capitalize_words
    replace: "good morning"
```

The first `$|$` marker is removed and the cursor is placed there after
expansion. `word`, `left_word`, and `right_word` follow Espanso's core boundary
model. With case propagation enabled, `;hello`, `;Hello`, and `;HELLO` produce
`good morning`, `Good Morning`, and `GOOD MORNING` respectively.

Press Backspace immediately after a plain, single-line expansion to restore its
trigger. Multiline and cursor-positioned expansions deliberately do not arm
undo because their cursor state is ambiguous.

## CLI

```text
snipexpand                       Run the daemon in the foreground
snipexpand init                  Create starter files without overwriting anything
snipexpand add TRIGGER TEXT      Add or overwrite a generated expansion
snipexpand remove TRIGGER        Remove a generated expansion
snipexpand list                  List loaded triggers, values, and source files
snipexpand check                 Validate configuration and report loaded counts
snipexpand detect                Show the focused application's properties
snipexpand reload                Ask the running daemon to reload immediately
snipexpand status                Query daemon health and active configuration
snipexpand status --json         Emit health information for scripts and widgets
snipexpand doctor                Diagnose the session, permissions, and runtime
snipexpand install               Install and enable the systemd user service
```

Use `\n` in the second argument to `add` for a multiline replacement:

```bash
snipexpand add ';mail' 'user@example.com'
snipexpand add ';sig' 'Best regards,\nSilouan'
snipexpand remove ';mail'
snipexpand check
```

Add and remove operations automatically notify a running daemon. Watched files
also reload automatically after manual edits, so a restart is unnecessary.
`reload` is available when an immediate, explicit refresh is useful.

## Current scope

SnipExpand intentionally rejects unknown configuration fields instead of silently
changing their meaning. The first release does not support regex triggers,
scripts or shell variables, forms, rich text, images, clipboard injection,
per-application match sets, imports, or an online package registry.

See the [compatibility matrix](docs/compatibility.md), the
[legacy TOML migration guide](docs/migration.md), and the
[Espanso-informed roadmap](docs/espanso-roadmap.md) for details.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build
cargo test
```

The end-to-end helper in `examples/e2e_type.rs` creates a temporary virtual
keyboard and requires access to `/dev/uinput`. It is intended for interactive
testing in a disposable text field; it is not run by the normal unit-test suite.

## License

MIT. See [LICENSE](LICENSE).
