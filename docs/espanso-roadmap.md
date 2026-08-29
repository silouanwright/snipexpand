# Espanso-informed roadmap

This roadmap is based on an architectural review of Espanso at commit
`fb3f825` (2026-08-14). This is a behavioral and architectural study, not a
code-porting plan.

## What Espanso's architecture gets right

Espanso separates input detection, matching, rendering, and output dispatch.
Application detection selects an active configuration; the matcher emits a
match event; extensions render variables; a dispatcher chooses event or
clipboard injection. Middleware handles concerns such as modifier release and
undo. These boundaries explain much of Espanso's capability, but its
cross-platform UI and package ecosystem also account for substantial
complexity that a Wayland-native tool does not need.

SnipExpand should preserve four small boundaries inside one crate:

1. Detect keyboard and active-application state.
2. Match static or future regex triggers without rendering side effects.
3. Render safe, declared variables.
4. Dispatch through an explicit injection strategy.

## Adopt next

| Capability | Why | Intended scope |
| --- | --- | --- |
| Active-app detection and exclusions | Prevent expansions in password managers and incompatible apps | Class, title, and executable regexes; diagnostic command |
| Case propagation | High-frequency convenience with small implementation cost | Espanso-compatible `propagate_case` and casing styles |
| Arbitrary Unicode injection | Active XKB layouts cannot represent every useful character | Implemented with persistent modifier-free Wayland text keymaps and a `wtype` fallback |
| Initialization | A good first run should not require reading source docs | Create minimal config and examples without overwriting files |
| Runtime diagnostics | Wayland failures otherwise look mysterious | Implemented as `snipexpand doctor`, alongside `snipexpand detect` |
| Backspace undo | Correcting an accidental expansion should be frictionless | Implemented for the immediately preceding plain, single-line expansion |

## Consider later

| Capability | Decision pressure |
| --- | --- |
| Clipboard backend for long text | Faster and more complete Unicode, but clipboard preservation and password-manager behavior need careful design |
| Regex triggers and capture variables | Powerful for structured input, but require bounded buffers and predictable deletion semantics |
| Per-app overrides beyond exclusion | Useful for injection timing and match sets after exclusions prove the detector |
| Nested safe variables | Useful once the rendering boundary is explicit; dependency cycles need validation |
| Package import/export | Start with ordinary directories or Git before building a registry |
| Search metadata | Add Espanso-compatible `search_terms` now that labels and the Omarchy search plugin exist |
| Pause and resume | The existing IPC and plugin make explicit controls inexpensive; defer a global keyboard gesture |
| Configurable word separators | Small compatibility improvement for punctuation, programming, and non-English text |

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
