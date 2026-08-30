# Research handoff: snippet packs

## Goal

Recommend a minimal, safe system for publishing and installing reusable
SnipExpand snippet packs.

## Current conclusion

Use ordinary Git repositories as the publishing mechanism and expose them as
read-only named groups through a small pack CLI. Build named groups first. Do
not build a registry, executable hooks, dependency resolution, or automatic
background updates. Auto-detect existing Espanso pack repositories and install
them only when the entire pack passes SnipExpand's strict compatibility checks.

## Next steps

1. Resolve the four decisions in `gaps.md`.
2. Design named groups and their persisted enable state.
3. Draft the `pack.yml` schema and CLI acceptance tests before implementation.

Read `findings.md`, `source-ledger.md`, and `gaps.md` before continuing.
