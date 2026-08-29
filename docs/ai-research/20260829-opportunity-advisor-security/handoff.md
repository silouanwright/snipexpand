# Research handoff: snippet opportunity advisor security

## Goal

Determine whether SnipExpand should build a privacy-preserving advisor that
reports configured snippets the user manually types instead of invoking.

## Current conclusion

Do not build general phrase discovery. An opt-in exact missed-snippet advisor
is defensible only if it retains no arbitrary typed text or hashes, persists
only opaque aggregate counters, exposes clear reset/disable controls, and is
preceded by removal of character-level debug logging. Application filtering
cannot guarantee password-field protection on the current `evdev` path.

## Important source files

- `source-ledger.md`
- `findings.md`
- `gaps.md`

## Next steps

1. Decide whether the product value justifies the narrowly scoped feature.
2. Write an implementation specification and explicit invariants before code.
3. Remove character-level debug logging independently.
4. Prototype only the streaming exact matcher and aggregate counter store, with
   tests proving no typed content, application identity, or event timestamps are
   persisted.

## Resume prompt

Read this handoff and the cited files first. Continue only the scoped security
and product-value research. Do not inspect or collect personal typed-content
logs.
