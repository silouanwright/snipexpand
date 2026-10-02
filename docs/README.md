# SnipExpand documentation

## Start here

- [Product README](../README.md): setup, commands, snippet syntax, and limitations.
- [TASKS.md](../TASKS.md): prioritized backlog and implementation status.
- [Compatibility](compatibility.md): supported Espanso subset and deliberate differences.
- [Editor setup](../schemas/README.md): export and associate YAML schemas.
- [Snippet packs](packs.md): publishing and managing native/compatible packs.

## Current work — 2026-10-02

- [Completed batch and audit](completion-authoring-batch-2026-10-02.md): 125 passing
  tests, local commits, verification, and later work.
- [Personal groups](group-contract.md): configuration, persistence, and IPC.
- [Interim reliability/echo checkpoint](checkpoint-reliability-2026-10-02.md):
  historical progress before groups were implemented.

- [Authoring checkpoint](checkpoint-authoring-2026-10-01.md): completed local
  commit `fc90d52`, 89 passing tests, scope, and release limitations.
- [Next-work plan](next-work-2026-10-02.md): automated compatibility testing,
  bounded echo support, then named personal groups.
- [Feature research](ai-research/20261001-autonomous-feature-priorities/findings.md):
  rationale and [sources](ai-research/20261001-autonomous-feature-priorities/source-ledger.md).
- [Espanso-informed roadmap](espanso-roadmap.md): architectural background with
  current delivery status. Older research folders are historical evidence.

The authoring features are committed locally on `codex/snippet-authoring` and
have not been pushed, installed, or released. Public daemon/plugin v0.4.1 contains
the earlier keyboard-access and Restart fixes. Unfinished Unicode/Fcitx5 work
remains in a separate checkout; its live validation is not covered by this offline suite.

- [Variables and rendering limits](variables.md): echo dependencies and escaping.
- [Generated reliability tests](testing/generated-suite.md): campaigns and reproduced fixes.
