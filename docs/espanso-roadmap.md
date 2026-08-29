# Espanso-informed roadmap

This roadmap is based on an architectural review of Espanso at commit
`3c4a281` (2026-08-24). This is a behavioral and architectural study, not a
code-porting plan.

## What Espanso's architecture gets right

Espanso separates input detection, matching, rendering, and output dispatch.
Application detection selects an active configuration; the matcher emits a
match event; extensions render variables; a dispatcher chooses event or
clipboard injection. Middleware handles concerns such as modifier release and
undo. These boundaries explain much of Espanso's capability, but its
cross-platform UI and package ecosystem also account for substantial
complexity that a Wayland-native tool does not need.

SnipExpand preserves four small boundaries inside one crate:

1. Detect keyboard and active-application state.
2. Match static and regex triggers without rendering side effects.
3. Render safe, declared variables.
4. Dispatch through an explicit injection strategy.

## Implemented foundation

| Capability | Scope |
| --- | --- |
| Active-app detection and exclusions | Title, class, and executable regexes with `snipexpand detect` diagnostics |
| Case propagation | Espanso-compatible `propagate_case` and casing styles |
| Arbitrary Unicode injection | Persistent modifier-free Wayland text keymaps with a `wtype` fallback |
| Initialization | Minimal configuration and examples without overwriting existing files |
| Runtime diagnostics | `snipexpand doctor`, `detect`, `check`, and `status` |
| Backspace undo | Immediate restoration of the preceding plain, single-line expansion |
| Regex triggers and capture variables | Configurable bounded buffer and named captures |
| Per-app overrides beyond exclusion | First-match profiles for match files and behavioral overrides |
| Nested safe variables | Missing-reference, ambiguity, and cycle validation |
| Search metadata | Espanso-compatible `search_terms` exposed to the Omarchy search plugin |
| Pause and resume | IPC, CLI, status JSON, and Omarchy plugin controls |
| Configurable word separators | Optional boundary override with the Unicode-aware default preserved |
| Duplicate triggers | Profile disambiguation for automatic matching and source-selectable picker insertion |

## Consider next

| Capability | Decision pressure |
| --- | --- |
| Espanso migration | Easier onboarding, but unsupported fields must be reported without rewriting the source configuration |
| Named snippet groups | Useful organization and quick control once real configurations become difficult to manage |
| Clipboard backend for long text | Faster long replacements, but clipboard preservation and password-manager behavior need careful design |
| Package import/export | Start with ordinary directories or Git before building a registry |

See the [prioritized backlog](../TASKS.md) for the current ordering and detailed
acceptance criteria.

## Defer or reject by default

- Shell and arbitrary script variables: large security and reproducibility cost.
- Forms and choice windows: introduce a GUI toolkit, focus restoration, and a
  second interaction model.
- Image, HTML, and Markdown injection: clipboard-format and application-specific
  complexity outside the core text-expansion promise.
- A hosted package registry: governance and supply-chain burden before local
  packaging has proven insufficient.
- Usage statistics and cross-platform backends: not part of the current
  Wayland-native product advantage.

## Lessons from Espanso implementation

- App filters use regular expressions over title, class, and executable; all
  fields in one filter are conjunctive, while multiple filters are alternatives.
- Injection timing is application-dependent and belongs in configuration.
- Modifier keys must be released or isolated before synthetic typing.
- Arbitrary Unicode and long text motivate a second backend rather than ever
  more elaborate key synthesis.
- Undo becomes ambiguous after cursor movement, mouse activity, rich formats,
  or application changes; a deliberately narrow implementation is safer.
- Config validation should reject ineffective combinations instead of logging a
  warning and continuing with surprising behavior.
