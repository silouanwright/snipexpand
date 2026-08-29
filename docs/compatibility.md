# Espanso Core Match Compatibility

SnipExpand implements a documented subset of Espanso's match-file format. It does not
claim full Espanso compatibility. `snipexpand check` rejects unsupported fields rather
than silently changing their behavior.

## Supported

| Feature | Status | Notes |
| --- | --- | --- |
| `trigger` | Full | Static triggers |
| `triggers` | Full | Multiple triggers share one replacement |
| `replace` | Full | Persistent Wayland text keymaps support configured Unicode; multiline YAML strings supported |
| `label` | Full | Human-readable name exposed to snippet browsers such as the Omarchy plugin |
| `search_terms` | Full | Additional metadata exposed by `list --json` for snippet search |
| `$|$` | Full | First cursor marker controls final cursor position |
| `word` | Core | Requires both left and right word boundaries |
| `left_word` | Core | Unicode alphanumerics and `_` count as word characters |
| `right_word` | Core | The typed separator is preserved |
| `propagate_case` | Full | Case-insensitive trigger with replacement casing |
| `uppercase_style` | Full | `uppercase`, `capitalize`, or `capitalize_words` |
| `global_vars` | Date | Applied to every match in the file |
| match `vars` | Date | Applied after global variables |
| date `format` | Full | Chrono/strftime formatting |
| date `offset` | Full | Signed offset in seconds |
| nested `match` variable | Full | References another trigger; missing references and cycles are rejected |
| Multiple files | Full | Recursive `.yml` and `.yaml` discovery |

## SnipExpand-specific settings

`config.yml` is not an Espanso base configuration. It currently accepts:

```yaml
trigger_mode: immediate # or space
terminators: [space]    # any combination of space, enter, tab
word_separators: [" ", ".", ","] # optional boundary override
injection_backend: auto # auto, wayland, or uinput; restart after changing
injection_delay_ms: 1   # 0 to 50; shared fallback
wayland_injection_delay_ms: 0 # tested Omarchy/Hyprland default
uinput_injection_delay_ms: 1  # optional backend-specific override
injection_settle_ms: 10 # 0 to 100; one-time pause before trigger deletion
undo_enabled: true      # immediate Backspace restores a plain expansion's trigger
app_exclusions:         # regex filters; entries are OR, fields are AND
  - class: "^1Password$"
```

## Unsupported and rejected

- Regex triggers
- Shell, script, clipboard, random, echo, choice, and form variables
- Forms, images, HTML, and Markdown effects
- Imports and anchors
- Espanso per-app config overrides (SnipExpand supports global app exclusions)
- Per-match injection backends
- Espanso Hub package metadata

These features may be added individually, but acceptance requires compatible
behavior and tests. Unknown fields are errors.

## Runtime limits

SnipExpand detects physical keyboard input through evdev. `auto` prefers
Wayland virtual-keyboard injection and falls back to uinput. Configured
replacement characters are mapped across persistent modifier-free Wayland text
keyboards at startup and rebuilt after configuration reloads. It cannot infer
whether a browser currently focuses a password field. IME/Fcitx5-transformed
text can differ from the raw keys observed by SnipExpand.

In immediate mode, `snipexpand check` warns when a shorter trigger makes a
longer trigger unreachable.
