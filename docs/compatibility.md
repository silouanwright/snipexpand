# Espanso Core Match Compatibility

SnipExpand implements a documented subset of Espanso's match-file format. It does not
claim full Espanso compatibility. `snipexpand check` rejects unsupported fields rather
than silently changing their behavior.

## Supported

| Feature | Status | Notes |
| --- | --- | --- |
| `trigger` | Full | Static triggers |
| `triggers` | Full | Multiple triggers share one replacement |
| `replace` | Full | Unicode falls back to compositor-native `wtype`; multiline YAML strings supported |
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
| Multiple files | Full | Recursive `.yml` and `.yaml` discovery |

## SnipExpand-specific settings

`config.yml` is not an Espanso base configuration. It currently accepts:

```yaml
trigger_mode: immediate # or space
terminators: [space]    # any combination of space, enter, tab
injection_delay_ms: 2   # 0–50; raise if an application drops injected keys
injection_settle_ms: 10 # 0–100; one-time pause before trigger deletion
undo_enabled: true      # immediate Backspace restores a plain expansion's trigger
app_exclusions:         # regex filters; entries are OR, fields are AND
  - class: "^1Password$"
```

## Unsupported and rejected

- Regex triggers
- Shell, script, clipboard, random, echo, choice, and form variables
- Forms, images, HTML, and Markdown effects
- Imports and anchors
- Search metadata
- Espanso per-app config overrides (SnipExpand supports global app exclusions)
- Per-match injection backends
- Espanso Hub package metadata

These features may be added individually, but acceptance requires compatible
behavior and tests. Unknown fields are errors.

## Runtime limits

SnipExpand detects physical keyboard input through evdev and normally injects through uinput.
It cannot infer whether a browser currently focuses a password field. Characters
absent from the active XKB layout use `wtype`, which must be installed and supported
by the compositor. IME/Fcitx5-transformed text can differ from the raw keys observed
by SnipExpand.
