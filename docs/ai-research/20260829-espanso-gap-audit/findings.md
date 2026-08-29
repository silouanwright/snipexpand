# Espanso gap audit findings

## Current SnipExpand surface

SnipExpand already covers the ordinary static-match core: one or several
triggers, plain and multiline replacements, labels, one cursor marker, word
boundaries, case propagation, date variables, recursive match files, config
reloads, undo, app exclusions, and explicit injection controls. The Omarchy
plugin now supplies search, insertion, editing, diagnostics, and restart UI.

## Useful gaps

1. `search_terms` is the closest follow-up to `label`. Espanso carries these
   terms into search without displaying them. SnipExpand can pass them through
   `list --json` to improve discovery with almost no daemon complexity.
2. Nested `match` variables provide composition without executing code or
   reading sensitive state. They need missing-reference and cycle validation,
   but otherwise fit the existing renderer.
3. Espanso exposes `enable`, `disable`, and `toggle` commands plus a double-tap
   modifier gesture. SnipExpand's existing IPC and plugin make explicit pause
   controls worthwhile. The global gesture can wait.
4. Espanso lets users configure `word_separators`. This is a small but useful
   improvement for punctuation-heavy triggers, code, and language-specific
   behavior.
5. Duplicate triggers open a chooser in Espanso. SnipExpand now allows them for
   picker workflows while refusing ambiguous typed expansion. App profiles can
   narrow a trigger to one active match, and `paste --source` selects an exact
   entry without arbitrary ordering.
6. Per-application profiles and bounded regex triggers with named captures are
   now implemented.

## Features that should stay later or out of scope

- `random` is safe but niche. `echo` is redundant with literal YAML and shared
  variables. Clipboard variables need explicit security treatment.
- `force_mode: clipboard` solves an Espanso backend problem, not a direct
  SnipExpand compatibility need. Per-app injection controls are more useful.
- Forms, choice windows, images, Markdown, and HTML require UI, clipboard
  formats, or application-specific behavior. They should not enter the daemon.
- Shell and script variables expand the trust boundary too far for the default
  product.
- Imports and anchors are lower value because recursive match files already
  organize reusable configuration. Git-based sharing can come first.

## Recommended order

All seven recommendations were implemented. Forms, rich text, executable
variables, and clipboard-backed behavior remain intentionally deferred.
