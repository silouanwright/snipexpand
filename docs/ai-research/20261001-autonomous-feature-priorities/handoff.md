# Research handoff: autonomous feature priorities

## Goal and plan
Rank useful next SnipExpand work by user benefit, Espanso compatibility,
implementation cost, and ability to validate without live desktop involvement.
Compare preview/diagnostics, variable extensions, dates, groups, and heavier UI
or injection features. Use official documentation, upstream issue reports,
and local source. Stop after each leading candidate has a concrete benefit,
verification path, and material limitation; no exhaustive popularity survey.

## Scope fence
Current lane: SnipExpand feature prioritization, October 1, 2026.
Allowed local roots: /home/silouan/Work/projects/snipexpand and
/home/silouan/Work/projects/snipexpand-omarchy (read-only source inspection).
This research packet is also preserved in the snipexpand-authoring checkout.
Documentation follow-up on 2026-10-02 may update this packet, the roadmap,
documentation index, next-work plan, and TASKS.md in either checkout.
Allowed web sources: Espanso official docs, upstream repositories/issues,
related primary library documentation.
Forbidden: unrelated project directories, personal snippet contents, live input
injection, desktop configuration changes, and edits to unfinished Unicode code.
If expected source is absent, record the gap rather than searching sibling projects.

## Status
Research complete. The first recommended batch was subsequently implemented
in commit `fc90d52`; the research findings remain a dated record of the decision.

## Completed conclusion
Prefer check/render diagnostics, editor schemas, then timezone-aware dates and
bounded echo support. Groups are a larger follow-up. Random/UUID are optional;
locales and interactive injection features are lower priority for this goal.

## Evidence and files
Read findings.md for rationale and acceptance criteria; source-ledger.md records
local code and official/upstream sources. gaps.md distinguishes uncertainties
from verified facts. TASKS.md links this researched ordering.

## Next steps

The first batch is complete locally; do not reimplement preview, schemas, or
IANA date timezones. Read the authoring checkpoint and
`docs/next-work-2026-10-02.md` for the proposed next batch: generated compatibility
tests and targeted fixes, then bounded echo support and personal snippet groups.
The existing live-input testing boundary still applies. No release has been made
from the authoring branch.

## Resume prompt
Read this handoff and findings.md, then only the cited source needed for the
chosen implementation. Use gaps.md for unresolved points. The scope fence above
applies; the chat transcript is not the canonical research record.


## Implementation follow-up — 2026-10-01

The user subsequently authorized a goal to implement the first batch. Checking,
offline previews, schemas/export/editor docs, and date timezones are now complete
in `/home/silouan/Work/projects/snipexpand-authoring` on `codex/snippet-authoring`.
Read that checkout's `docs/checkpoint-authoring-2026-10-01.md` for the tested
implementation. This research packet remains the rationale for later priorities.
