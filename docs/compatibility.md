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
| `global_vars` | Core | Date and nested-match variables applied to every match in the file |
| match `vars` | Core | Date and nested-match variables applied after global variables |
| date `format` | Full | Chrono/strftime formatting |
| date `offset` | Full | Signed offset in seconds |
| nested `match` variable | Full | References another trigger; missing references and cycles are rejected |
| Multiple files | Full | Recursive `.yml` and `.yaml` discovery |
| `regex` | Core | Suffix matching with named captures exposed as `{{name}}`; bounded by `regex_max_buffer` |
| Duplicate triggers | Core | Source-selectable through `paste`; automatic typing requires profile disambiguation |

## SnipExpand-specific settings

`config.yml` is not an Espanso base configuration. It currently accepts:

```yaml
trigger_mode: immediate # or space
terminators: [space]    # any combination of space, enter, tab
word_separators: [" ", ".", ","] # optional boundary override
regex_max_buffer: 256 # 32 to 4096 characters
injection_backend: auto # auto, wayland, or uinput; restart after changing
non_bmp_input: auto # auto, keymap, compose, or input_method
injection_delay_ms: 1   # 0 to 50; shared fallback
wayland_injection_delay_ms: 0 # tested Omarchy/Hyprland default
uinput_injection_delay_ms: 1  # optional backend-specific override
injection_settle_ms: 10 # 0 to 100; one-time pause before trigger deletion
compose_delay_ms: 5     # 0 to 50; between numeric Unicode compose keys
compose_settle_ms: 10   # 0 to 100; before deletion and around Unicode compose
undo_enabled: true      # immediate Backspace restores a plain expansion's trigger
app_exclusions:         # regex filters; entries are OR, fields are AND
  - class: "^1Password$"
app_profiles:           # first matching profile wins
  - name: Browser
    filter: { class: "firefox" }
    enabled: true
    include_match_files: [browser.yml]
    exclude_match_files: [browser/private.yml]
    trigger_mode: space
    terminators: [space, enter]
    word_separators: [" ", ".", ","]
    injection_delay_ms: 1
    injection_settle_ms: 10
    compose_delay_ms: 5
    compose_settle_ms: 10
    non_bmp_input: compose
```

Profile filters accept `title`, `class`, and `exec` regular expressions. A
profile can enable or disable expansion, select match files, and override the
listed matching and timing settings. The first matching profile wins.

## Unsupported and rejected

- Shell, script, clipboard, random, echo, choice, and form variables
- Forms, images, HTML, and Markdown effects
- Imports and anchors
- Espanso's separate per-app config files; SnipExpand uses `app_profiles`
- Per-match injection backends
- Espanso base-configuration fields outside SnipExpand's documented settings

These features may be added individually, but acceptance requires compatible
behavior and tests. Unknown fields are errors.

## Runtime limits

SnipExpand detects physical keyboard input through evdev. `auto` prefers
Wayland virtual-keyboard injection and falls back to uinput. Configured
replacement characters are mapped across persistent modifier-free Wayland text
keyboards at startup and rebuilt after configuration reloads. In `auto` mode,
Chromium and Electron applications use synchronized Unicode compose input for
characters above U+FFFF; other applications retain direct keymap input. An
application profile can force `keymap` or `compose`, which is useful for unusual
application packaging that does not expose its Chromium runtime. The opt-in
`input_method` mode atomically deletes the trigger and commits UTF-8 directly
when Wayland input-method-v2 is available and the focused client has an active
text-input-v3 context that confirms the exact trigger as surrounding text.
Before-commit failures fall back to compose for
Chromium/Electron and to the persistent keymap elsewhere. Post-commit failures
are not retried because the application may already contain the replacement.
It cannot infer
whether a browser currently focuses a password field. IME/Fcitx5-transformed
text can differ from the raw keys observed by SnipExpand.

Wayland permits only one input-method object per seat. SnipExpand therefore
never registers input-method-v2 in `auto`, `keymap`, or `compose` mode. Setting
`input_method` opts into seat ownership and requires a daemon restart. It is
normally unavailable on Omarchy while Fcitx5 owns the seat, and it is not a
replacement for Fcitx5. Non-Wayland and unsupported Wayland environments retain
the existing uinput or virtual-keyboard fallback.

Compose input is emitted by the persistent Wayland injector rather than a
per-character subprocess. `compose_delay_ms` controls pacing between the
Ctrl+Shift+U, hexadecimal, and Enter keys. `compose_settle_ms` protects trigger
deletion even when the general settle setting is lower, then gives the target
application time to enter and commit preedit state before surrounding text is
sent. These settings do not affect the normal persistent text-keymap path.

In immediate mode, `snipexpand check` warns when a shorter trigger makes a
longer trigger unreachable.
