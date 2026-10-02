# Research Handoff: Chromium/Electron Non-BMP Input

## Goal

Explain the Signal/Chromium non-BMP truncation bug, compare upstream and peer-project solutions, and evaluate SnipExpand's application-aware compose fallback.

## Current conclusion

Two defects are confirmed. Direct XKB input is narrowed by Chromium from a
32-bit character to `char16_t`, turning 🙂 (`U+1F642`) into private-use
`U+F642`. The current app-scoped Ctrl+Shift+U workaround avoids that narrowing,
but Signal intermittently remains in the visible `U+1f642` preedit state. The
workaround therefore needs explicit compose-specific sequencing and timing.

Recommended next implementation: keep the fast persistent direct keymap for
terminals and ordinary text; replace the per-character short-lived wtype
compose call with an injector-owned, paced compose sequence for
Chromium/Electron non-BMP characters. Add deterministic ordering, release,
routing, and configuration tests. A guarded Signal retest remains required and
must not run without fresh user approval.

## Implementation checkpoint

Implemented locally on 2026-09-02. The persistent Wayland injector now owns the
complete numeric Unicode compose sequence, resolves all required active-keymap
keys before injection, applies separate compose delay and settle settings, and
clears synthetic modifiers on success or failure. After Signal ignored trigger
deletion with the user's general settle set to zero, compose expansions were
also changed to use the safer of the general and compose settle intervals
before Backspace. Direct keymaps remain the default for terminals and normal
characters. Rust tests pass; guarded Chromium and Signal live retests remain
intentionally outstanding.

Medium-term research target: optional input-method-v2 `commit_string` for
text-input-v3 clients, with virtual-keyboard fallback and safe IME coexistence.

## Important source files

- `source-ledger.md`
- `findings.md`
- `gaps.md`

## Resume prompt

Read this handoff and the source ledger first. Continue only the Chromium/Electron non-BMP Unicode injection lane. Do not perform live input, clipboard, focus, or service actions.
