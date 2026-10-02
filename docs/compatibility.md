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
| `global_vars` | Core | Date, echo, and nested-match variables merged into every match in the file |
| match `vars` | Core | Local definitions override globals; dependencies determine evaluation order |
| date `format` | Full | Chrono/strftime formatting |
| date `offset` | Full | Signed elapsed seconds, checked for overflow |
| date `tz` | Core | IANA timezone names; invalid names are errors rather than Espanso's local-time fallback |
| nested `match` variable | Core | Exact references; missing/ambiguous references, cycles, and excessive depth are rejected |
| `echo` variable | Core | References in `params.echo`, escaping, and `inject_vars: false`; bounded output |
| Multiple files | Full | Recursive `.yml` and `.yaml` discovery |
| `regex` | Core | Suffix matching with named captures exposed as `{{name}}`; bounded by `regex_max_buffer` |
| Duplicate triggers | Core | Source-selectable through `paste`; automatic typing requires group/profile disambiguation |

## SnipExpand-specific settings

`config.yml` is not an Espanso base configuration. It currently accepts:

```yaml
trigger_mode: immediate # or space
terminators: [space]    # any combination of space, enter, tab
word_separators: [" ", ".", ","] # optional boundary override
regex_max_buffer: 256 # 32 to 4096 characters
injection_backend: auto # auto, wayland, or uinput; restart after changing
non_bmp_input: auto # auto, keymap, fcitx5, compose, or input_method
fcitx_sensitive_hint: allow # allow or suppress; Password is always blocked
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
    fcitx_sensitive_hint: suppress
```

Profile filters accept `title`, `class`, and `exec` regular expressions. A
profile can enable or disable expansion, select match files, and override the
listed matching and timing settings. The first matching profile wins. Group selection is always applied as well.
Snippets whose nested targets are filtered out are suppressed transitively,
including when a profile excludes the target. Static missing/cyclic references
remain validation errors.

## Unsupported and rejected

- Date `locale` overrides and variable injection outside `params.echo`
- Shell, script, clipboard, random, choice, and form variables
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
keyboards at startup and rebuilt after configuration reloads.

In `auto` mode, Chromium and Electron applications first ask SnipExpand's
optional Fcitx5 addon to verify and replace the exact trigger through the
focused input context. This direct UTF-8 commit avoids visible numeric Unicode
preedit for characters above U+FFFF while coexisting with the input method that
already owns the Wayland seat. A temporarily stale surrounding-text snapshot
gets one bounded retry. When a client such as Signal does not expose
surrounding text at all, the first refusal creates a 100-millisecond permit
bound to the same Fcitx input context, trigger, and replacement. The second
call can then forward the trigger's Backspaces and commit final UTF-8 through
that context without displaying numeric compose preedit. A changed focus,
request, selection, password field, or expired permit is refused. Other safe
refusals use synchronized Unicode compose; other applications retain direct
keymap input.

An application profile can force `keymap`, `fcitx5`, or `compose`, which is
useful for unusual application packaging that does not expose its Chromium
runtime. The separate opt-in `input_method` mode asks SnipExpand itself to own
input-method-v2 and atomically commit UTF-8 when the focused client confirms the
exact trigger as surrounding text. Before-commit failures fall back to compose
for Chromium/Electron and to the persistent keymap elsewhere. Indeterminate
post-dispatch failures are never retried because the application may already
contain the replacement. The addon refuses direct commits when Fcitx5 marks the
focused context as a password field. Fcitx's separate `Sensitive` hint disables
prediction and is also used by Signal's ordinary private-message composer, so
the default `fcitx_sensitive_hint: allow` policy permits expansion there. Set
the option to `suppress` globally or in an application profile for stricter
behavior. SnipExpand's app-level detector still cannot infer a browser field
type, and the daemon still observes global key events. IME/Fcitx5-transformed
text can differ from the raw keys observed by SnipExpand.

Wayland permits only one input-method object per seat. SnipExpand therefore
never registers input-method-v2 in `auto`, `keymap`, `fcitx5`, or `compose`
mode. Its Fcitx5 addon uses the existing owner's public input-context API
instead. Setting `input_method` opts into seat ownership and requires a daemon
restart. It is normally unavailable on Omarchy while Fcitx5 owns the seat, and
it is not a replacement for Fcitx5. Non-Wayland and unsupported Wayland
environments retain the existing uinput or virtual-keyboard fallback.

Compose input is emitted by the persistent Wayland injector rather than a
per-character subprocess. `compose_delay_ms` controls pacing between the
Ctrl+Shift+U, hexadecimal, and Enter keys. `compose_settle_ms` protects trigger
deletion even when the general settle setting is lower, then gives the target
application time to enter and commit preedit state before surrounding text is
sent. These settings do not affect the normal persistent text-keymap path.

In immediate mode, `snipexpand check` warns when a shorter trigger makes a
longer trigger unreachable.


## Offline authoring tools

`render` evaluates exact literal triggers (including nested and date variables)
without daemon IPC, keyboard input, or clipboard writes. It uses explicit source
and profile selection rather than active-window detection. It previews `paste`
behavior; it does not simulate word boundaries, typed case, or regex matching.
`schema config` and `schema match` export the supported YAML shapes for editors.
See [the editor guide](../schemas/README.md). Neither schemas nor preview replace
live application compatibility testing.

Date and nested-match variables reject irrelevant parameters. Date formats,
known timezones, and representable offset results are checked during loading;
runtime rendering also returns errors if the clock or result becomes invalid.
All dates in a single expansion use the same instant, including nested matches.

## Echo and dependency limits

The authoring branch supports `type: echo`, references inside `params.echo`,
`inject_vars: false`, and escaped opening braces. Dependency order, substitution,
and rendering limits are specified in [variables.md](variables.md). Schemas,
`check`, `render`, and pack inspection share the supported field shapes.

## Personal groups

`snippet_groups` configures named personal file/subtree collections. Preferences
persist separately and are managed through `group list/enable/disable/toggle`.
This is a SnipExpand extension. See [the group contract](group-contract.md) for
profile/pack/reference precedence, inventory metadata, and the IPC protocol.
