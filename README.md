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
- Regex triggers with named captures
- Reusable echo values, nested snippets, and date variables
- Configurable word boundaries and case propagation
- Search labels and terms for picker integrations
- Immediate Backspace undo for simple expansions
- Application exclusions, per-application profiles, and persistent personal groups
- Pause and resume controls through the CLI, IPC, and Omarchy plugin
- Deliberate duplicate-trigger selection by source
- Git-published snippet packs with pinned revisions and explicit updates
- Direct installation of strictly compatible Espanso packs
- Persistent Wayland injection with a `uinput` fallback
- Flash-free supplementary Unicode in Chromium and Electron through Fcitx5
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

Run SnipExpand from a local Wayland desktop session with:

- `libxkbcommon` and Wayland client libraries
- Read access to keyboard devices under `/dev/input/` (see setup below)
- `wtype` for the Unicode fallback path

On systems using Fcitx5, `snipexpand install` also builds a small per-user
bridge for flash-free emoji and other Unicode above U+FFFF in Chromium and
Electron. This optional bridge uses `busctl` at runtime and needs `c++`,
`pkg-config`, and the Fcitx5 development files at installation time. SnipExpand
keeps working through its compose fallback when they are unavailable. `doctor`
checks that the loaded addon matches the current binary. After an upgrade, an
older addon can remain in memory until Fcitx5 restarts; on Omarchy use
`omarchy restart xcompose`, then run `snipexpand doctor` again. Other systems
can start a new desktop session.

## Install

Install from crates.io:

```bash
cargo install snipexpand
```

Prebuilt x86_64 and aarch64 binaries are available from
[GitHub Releases](https://github.com/silouanwright/snipexpand/releases).

## Set up

```bash
snipexpand install --keyboard-access
snipexpand doctor
```

Run these commands as your desktop user. The `--keyboard-access` option requests
administrator authentication to install a keyboard-only udev rule. The rule
grants keyboard access to the active local session and applies again after a
reboot or keyboard reconnect. It does not add your user to the `input` group.

Setup creates missing starter files without overwriting your config, installs
the optional Fcitx5 bridge when its build tools are available, and starts and
enables the user service. It checks that a keyboard can actually be read before
reporting success. If your system already provides keyboard access, you can
use `snipexpand install` without the flag.

### Manual keyboard permissions

On a system using systemd-logind and udev, an administrator can install the same
rule manually. This also works with older SnipExpand releases that do not have
`--keyboard-access`:

```bash
sudo tee /etc/udev/rules.d/71-snipexpand-keyboard.rules >/dev/null <<'EOF'
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_KEYBOARD}=="1", TAG+="uaccess"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger --action=change --subsystem-match=input --property-match=ID_INPUT_KEYBOARD=1 --settle
systemctl --user restart snipexpand
```

Package maintainers can ship `contrib/71-snipexpand-keyboard.rules` in their
distribution's udev rules directory. The CLI embeds the rule, so binaries and
crates.io installs can configure it through `--keyboard-access` too. The automatic
setup refuses to overwrite a different existing rule at its destination.

If your system does not support active-session udev access, follow its input
device permission policy. Membership in the `input` group is another option,
but grants access to additional input devices and requires a new login.

### Service runs, but typed triggers do not expand

Run `snipexpand doctor`. A running service or a successful paste from the Omarchy
panel only confirms that SnipExpand can send text. Automatic expansion also
requires keyboard read access. Restarting the service cannot repair a missing
permission. Current diagnostics test actual keyboard access; older releases
check only `input` group membership and may report failure even with a working
udev rule.

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
snipexpand add --label 'Email address' --search-term contact ';mail' 'user@example.com'
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
    label: "Email address"
    search_terms: [email, contact]
    replace: "user@example.com"

  # Whole-word matching and multiline text
  - trigger: ";sig"
    label: "Email signature"
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

  # Named regex captures become replacement variables.
  - regex: "issue-(?P<number>\\d{3})"
    replace: "Issue #{{number}}"

  # Reuse another snippet without running commands.
  - trigger: ";name"
    replace: "Your Name"

  - trigger: ";greeting"
    replace: "Hello from {{name}}"
    vars:
      - name: name
        type: match
        params:
          trigger: ";name"
```

## Personal snippet groups

Define collections in `config.yml` using paths relative to `match/`:

```yaml
snippet_groups:
  - name: work
    match_files: [work, signatures.yml]
    enabled: true
```

```sh
snipexpand group list --json
snipexpand group disable work
snipexpand group enable work
snipexpand group toggle work
```

Changes persist in `groups.json`, preserving your YAML comments. Commands update
an available daemon or save preferences for its next start. Group selection
applies to automatic expansion, `paste`, and offline `render`; source selection
cannot bypass it. App profiles can further restrict snippets. Dependents of an
inactive nested snippet are also inactive until their targets become available.
Packs retain their separate controls. See [personal group behavior and IPC](docs/group-contract.md)
for overlap, counts, persistence failures, and profile/pack precedence.

## Preview and validate snippets

```bash
snipexpand check
snipexpand render ';greeting'
snipexpand render ';function' --json
snipexpand render ';mail' --source ~/.config/snipexpand/match/work.yml
snipexpand render ';mail' --profile Work
```

`render` evaluates an exact configured literal trigger through the same renderer
used by `paste`. It writes the replacement without an added newline, removes the
first `$|$` cursor marker, and never types or changes the clipboard. JSON output
includes `text`, `source`, `profile`, `cursor_position` (from the start), and
`cursor_back` (from the end). Cursor offsets count Unicode characters, not bytes
or visual columns.

Use `--source` or `--profile` to resolve duplicates. Source paths may be absolute
or relative to the current directory. A profile is selected by name without
examining the focused window; a disabled profile produces an error. Without
`--profile`, enabled groups and their available dependencies are considered. This previews insertion,
not the typed matching process: word boundaries, typed-case propagation, app
exclusions, and regex sample input are not simulated.

`check`, `render`, and `schema` do not create starter files or require a running
daemon. On an empty configuration, `check` reports zero matches and `render`
reports a missing trigger. Use `init` to create starter configuration.

### Dates and timezones

```yaml
matches:
  - trigger: ';utc'
    replace: '{{stamp}}'
    vars:
      - name: stamp
        type: date
        params:
          format: '%Y-%m-%dT%H:%M:%SZ'
          tz: UTC
          offset: 0
```

`tz` accepts IANA names such as `UTC`, `America/Chicago`, and `Europe/Paris`.
Omit it to use system local time. Invalid zone names, invalid date formats, and
out-of-range offsets are errors. Date variables, including nested snippets,
share one captured instant per expansion. Offsets are elapsed seconds: `86400`
is 24 hours, which can differ from the same local time tomorrow across a
daylight-saving transition. Full locale overrides remain unsupported.

Date variables accept `format`, `offset`, and `tz`; nested-match variables accept
`trigger`; echo variables accept `echo`. Variable names use letters, numbers, or underscores. Unsupported
parameters are rejected, including on unused global definitions, so `check`
catches mistakes before expansion. If rendering later fails, automatic expansion
leaves the typed trigger intact and logs the error.

### Reusable echo variables

Define `type: echo` with `params: {echo: 'Your text'}` to reuse a value.
Echo parameters support references to other variables, with dependency checks,
literal-brace escaping, and bounded output. See [variables and rendering limits](docs/variables.md)
for examples and the supported Espanso subset.

### Editor autocomplete and validation

Use the bundled YAML schemas for completion and error checking:

```bash
snipexpand schema match   # Prints the match-file schema as JSON
snipexpand schema config  # Prints the settings schema as JSON
```

See [editor setup](schemas/README.md) for exporting schemas and associating them
with YAML files. `snipexpand check` performs additional cross-file and semantic
validation beyond the editor schema.

## Settings

Edit `~/.config/snipexpand/config.yml`:

```yaml
# Choose when expansion happens.
trigger_mode: space        # immediate | space
terminators: [space]       # any of: space, enter, tab

# Optional. Override which characters delimit word-boundary matches.
# word_separators: [" ", ".", ",", "!", "?"]

# Maximum characters retained while evaluating regex triggers.
regex_max_buffer: 256

# Prefer native Wayland injection and fall back to uinput.
injection_backend: auto    # auto | wayland | uinput

# Auto uses Fcitx5 for Chromium/Electron when available, then compose fallback.
non_bmp_input: auto         # auto | keymap | fcitx5 | compose | input_method

# fcitx5 uses the installed bridge explicitly. input_method instead asks
# SnipExpand to own the exclusive Wayland input-method-v2 seat itself.

# Allow ordinary fields carrying Fcitx's privacy/no-prediction hint.
# Actual password fields are always blocked.
fcitx_sensitive_hint: allow # allow | suppress

# Tune these only if an application drops or reorders characters.
injection_delay_ms: 1
wayland_injection_delay_ms: 0
uinput_injection_delay_ms: 1
injection_settle_ms: 10

# Chromium/Electron Unicode compose timing. This does not slow normal text.
compose_delay_ms: 5
compose_settle_ms: 10  # protects trigger deletion and compose transitions

# Backspace immediately after a simple expansion to restore its trigger.
undo_enabled: true         # true | false

# Optional. Disable expansion in matching applications. Default: []
app_exclusions:
  - class: "^1Password$"
  - class: "^org\\.keepassxc\\.KeePassXC$"

# Optional. The first matching profile overrides behavior for that application.
app_profiles:
  - name: Browser
    filter:
      class: "^(firefox|chromium)$"
    include_match_files: [browser.yml]
    trigger_mode: space
    injection_delay_ms: 1
    # non_bmp_input: compose # override for unusual application packaging
    # fcitx_sensitive_hint: suppress
    # compose_delay_ms: 5
    # compose_settle_ms: 10
```

Run `snipexpand detect` while an application is focused to find the title,
class, and executable values needed for an exclusion or profile.

## Snippet packs

Install a native SnipExpand pack or a compatible Espanso pack directly from a
Git repository:

```bash
snipexpand pack inspect espanso:arrows
snipexpand pack install espanso:arrows

snipexpand pack inspect https://github.com/example/useful-symbols
snipexpand pack install https://github.com/example/useful-symbols
```

Use `--path DIR` when a repository contains several packs and `--ref REF` to
select a tag, branch, or commit. SnipExpand records the exact resolved commit.
Installed packs are read-only, independently enableable collections. Updates
are always explicit:

```bash
snipexpand pack list
snipexpand pack disable useful-symbols
snipexpand pack enable useful-symbols
snipexpand pack update useful-symbols
snipexpand pack remove useful-symbols
```

SnipExpand validates every pack before enabling it. Espanso packs using
unsupported fields, scripts, forms, or variables are rejected rather than
partially installed.
The `espanso:NAME` shorthand selects the latest stable version of a compatible
package from the official Espanso Hub.

## Commands

```text
snipexpand [COMMAND]
sxp [COMMAND]                    Short form

(no command)                     Run the daemon in the foreground
init                             Explicitly create starter configuration
add TRIGGER TEXT                 Add or replace a generated expansion
remove TRIGGER                   Remove a generated expansion
list                             List triggers and source files
check                            Validate configuration without creating files
render [--source PATH] [--profile NAME] [--json] TRIGGER
                                Preview a literal snippet without typing
schema config|match              Print a bundled YAML editor schema
detect                           Inspect the focused application
reload                           Reload the running daemon
enable                           Enable automatic expansion
disable                          Pause automatic expansion
toggle                           Toggle automatic expansion
paste [--source PATH] [--delay-ms N] TRIGGER
                                Insert a configured expansion
status [--json]                  Show daemon and configuration status
doctor                           Diagnose setup and runtime requirements
install                          Install and start the user service
uninstall                        Remove the service; preserve configuration
group list [--json]              List personal collections and availability
group enable|disable|toggle NAME Change a persistent collection preference
pack inspect SOURCE              Validate a Git-published pack
pack install SOURCE              Install and enable a pack
pack list                        List installed packs
pack update NAME                 Update a pack explicitly
pack enable NAME                 Enable an installed pack
pack disable NAME                Disable an installed pack
pack remove NAME                 Remove an installed pack
```

Duplicate triggers may coexist for picker use. Automatic typing expands only
when enabled groups and the active app profile leave one matching snippet. A picker can select an
exact duplicate with `paste --source PATH TRIGGER`, using the `source` returned
by `list --json`.

## Limitations

- Hyprland is the only supported and tested compositor. Other Wayland
  compositors may work but are not yet part of the test matrix.
- Undo works only immediately after a plain, single-line expansion. Multiline
  and cursor-positioned expansions cannot be undone back to their trigger.
- SnipExpand does not run scripts or shell commands, display forms, insert rich
  text or images, or provide a package registry.
- Regex triggers use a bounded rolling buffer and support named captures, but
  do not implement Espanso's full regex behavior.
- Variables are limited to echo text, formatted dates, regex captures, and safe
  nested snippet references. Shell, script, and form variables are intentionally
  unsupported.
- Application exclusions operate at the application level. Wayland does not
  expose a browser's focused field type, so SnipExpand cannot automatically
  identify password fields inside an allowed browser. The optional Fcitx5
  bridge does refuse direct commits when the input method marks a field as a
  password field. Fcitx's broader `Sensitive` hint is allowed by default
  because private-message composers such as Signal use it for ordinary text.
  Set `fcitx_sensitive_hint: suppress` globally or in an application profile
  for a stricter policy. This does not stop global keyboard-event reading.
- `non_bmp_input: input_method` requires Wayland input-method-v2, an active
  text-input-v3 client that reports surrounding text, and exclusive ownership
  of the seat's input-method slot. It falls back to the keyboard path when
  unavailable. Omarchy runs Fcitx5 by default, so keep `auto` unless Fcitx5 is
  intentionally absent.
- The optional Fcitx5 bridge verifies the exact suffix when the focused
  application supplies surrounding text. For applications such as Signal that
  do not, it allows one short-lived, same-field fallback that forwards the
  trigger deletion and commits the final UTF-8 text through Fcitx5. If either
  check is refused, `auto` falls back to Unicode compose, which may briefly
  show its `U+...` preedit.
- SnipExpand reads global keyboard events, including sensitive input.
  Application exclusions stop expansion but do not stop the daemon from
  receiving those events. Install only binaries you trust.

The daemon does not execute snippet content, access the clipboard, or make
network requests. Pack-management commands contact only the Git remotes you
explicitly request.

See the [compatibility matrix](docs/compatibility.md) for the complete supported
configuration surface.

## Documentation

- [Prioritized tasks](TASKS.md)
- [Espanso compatibility](docs/compatibility.md)
- [Espanso-informed design notes](docs/espanso-roadmap.md)
- [Publishing and managing snippet packs](docs/packs.md)

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
