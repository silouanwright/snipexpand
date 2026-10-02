# Source ledger

Reviewed 2026-10-01. Tier 1 means official documentation or primary source.
Upstream issue bodies are firsthand user requests, not evidence of prevalence
or independently verified defects. Issue state was checked through GitHub CLI.

| ID | Source | Evidence and limits |
| --- | --- | --- |
| L1 | `src/config.rs`: VariableKind, VariableParams, Config::validate, merge_variables | Only Date and Match kinds. Local/global merging exists. Date formats and offset ranges lack explicit validation. No schema files found in scoped repository search. |
| L2 | `src/expander.rs`: render, expansion_for_trigger | Date rendering uses local time and seconds offset. The renderer is separable from injection; replacement uses ordered string substitution. Source-selectable literal-trigger rendering exists internally. |
| L3 | `src/main.rs`, `src/packs.rs`, `Cargo.toml` | No render CLI. Strict pack inspection and enable/disable already exist. Chrono uses clock feature; no named timezone data dependency. |
| L4 | Plugin `Panel.qml`, `SnipExpandController.qml` | Existing list previews use stored replacement text; scoped search found no copy action. A rendered preview would be distinct from this existing text preview. |
| L5 | `TASKS.md`, `docs/espanso-roadmap.md`, prior gap audit and Fcitx handoff | Most earlier high-value Espanso gaps are done. Live Unicode validation remains gated; code has mixed unpublished work. |
| S1 | [Espanso extensions](https://espanso.org/docs/matches/extensions/) — Tier 1 | Echo, random choices, date locale and timezone support. Timezones documented from v2.3.0; invalid zone falls back locally upstream. Does not establish how often features are used. |
| S2 | [Espanso variables](https://espanso.org/docs/matches/variables/) — Tier 1 | Parameter interpolation, escaped brackets, local/global dependency ordering, inject_vars and depends_on complicate broad compatibility claims. |
| S3 | [Official match schema](https://github.com/espanso/espanso/blob/dev/schemas/match.schema.json) — Tier 1 | Upstream maintains machine-readable editor schema. SnipExpand must describe its own accepted subset rather than reuse this broader schema blindly. |
| S4 | [Schema contribution #1721](https://github.com/espanso/espanso/issues/1721) — primary user request | Explicit desire for type checking/validation in complex configurations; closed. |
| S5 | [Timezone request #2225](https://github.com/espanso/espanso/issues/2225) — primary user request | Concrete UTC timestamp need, previously requiring a script; closed. |
| S6 | [Timezone request #2186](https://github.com/espanso/espanso/issues/2186) — primary user request | Another request for output in UTC independent of system timezone; closed. |
| S7 | [Organizing matches](https://espanso.org/docs/matches/organizing-matches/) — Tier 1 | File-based organization is already a valid alternative to additional group state. |
| S8 | [Groups request #933](https://github.com/espanso/espanso/issues/933) — primary user request | Requests saved groups and labels, with LaTeX examples; also asks for unrelated before/after actions. Does not establish demand for runtime group toggles specifically; closed. |
| S9 | [UUID package](https://hub.espanso.org/uuid/) — primary package documentation | A practical UUIDv4 use case exists, currently implemented through a Python-dependent package. A native variable does not automatically make this script-based package compatible. |
| S10 | [Copy selection request #2809](https://github.com/espanso/espanso/issues/2809) — primary user request | Requests deliberate copy instead of insertion due to focus behavior on macOS; open. Treat motivation as that reporter's experience, not a demonstrated SnipExpand/Linux bug. |
| S11 | [Chrono docs](https://docs.rs/chrono/latest/chrono/index.html) — Tier 1 | UTC/local/fixed offsets are available; full named-zone support requires data from a companion implementation. Locale API is marked unstable. |
| S12 | [Chrono-TZ docs](https://docs.rs/chrono-tz/latest/chrono_tz/) — Tier 1 | Named timezones and daylight-saving behavior have a standard library implementation; adds dependency/data cost. |
| S13 | [Chrono format docs](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) — Tier 1 | Authoritative format syntax for validation and examples. |
| S14 | [Espanso forms](https://espanso.org/docs/matches/forms/) — Tier 1 | Forms are an interactive input flow, not just a renderer addition. |

No popularity score was inferred from issue counts, search ranking, or stars.
No personal configuration, clipboard, or live input was read during research.
