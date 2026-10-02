# Personal groups — implementation contract

Status: design for the unfinished part of the active 2026-10-02 goal. This is
not a claim that group commands already exist. Continue on `codex/snippet-authoring`.

## Configuration and selection

Proposed settings syntax:

```yaml
snippet_groups:
  - name: work
    match_files: [work, signatures.yml]
    enabled: true
  - name: greek
    match_files: [languages/greek.yml]
    enabled: false
```

Names are unique and stable, with letters, numbers, `_`, and `-`. Paths are
relative to `match/`, refer to a file or directory subtree, and cannot be empty,
absolute, contain `..`, or select `packs/`. Reject invalid paths/names and empty
membership lists. Paths may be temporarily absent so adding a new personal file
does not require editing the group definition again.

Ungrouped snippets remain active. A snippet belonging to several groups is
available only when all of them are enabled. File membership uses paths relative
to the loaded configuration root, not ambiguous filename suffixes.

Apply group filters together with the selected app profile. A disabled profile
still disables all expansion; an enabled profile cannot re-enable a disabled
group. After filtering, remove snippets whose nested-match targets are inactive,
transitively. Disabling one group therefore never makes the whole configuration
invalid just because another snippet references it. Enabling restores those
snippets when all dependencies are available. Static missing/cyclic/ambiguous
references still fail validation, including in disabled groups. The existing
profile-reference policy may be made consistent with this rule if documented
and covered by regression tests.

Automatic matching, explicit paste, and offline render use this same selection.
Source selection cannot bypass disabled groups. Plain duplicate triggers remain
ambiguous only among active entries; disabling a collection can disambiguate
them. Nested references retain exact, globally unambiguous trigger semantics.

Installed packs remain controlled by `pack enable/disable`, independent of
personal groups. Pack mirrors cannot be included in a personal group. A personal
snippet with a reference to a removed/disabled pack keeps the existing pack
validation behavior: pack disable must not silently break the configuration.

## Commands, persistence, and IPC

- `group list [--json]`
- `group enable NAME`, `group disable NAME`, `group toggle NAME`
- Prefer corresponding daemon IPC operations while running. If no daemon can be
  connected, apply the same persistence transition offline.
- Never retry a toggle offline after a request may have reached the daemon:
  a lost response must not cause a second toggle.
- JSON IPC uses an explicit operation and name, with structured response data;
  handle errors without terminating the daemon.

Keep user YAML and comments intact. Store overrides in a separate versioned
`groups.json` under the configuration directory, with atomic same-directory
replacement and an exclusive lock around read/modify/write. Defaults come from
`config.yml`; an override wins. Ignore unknown saved names for selection but
preserve them so a temporarily removed definition can recover its prior state.
Invalid state files fail validation instead of silently resetting preferences.

Load and validate a candidate before committing an override. If persistence
fails, the running matcher and previous state file must remain unchanged. After
persistence succeeds, install that exact candidate in memory; do not re-read a
different candidate. Cancel partial input, pending expansion, and undo on a
successful transition. The recent reload helper provides a seam for these tests.
Concurrent offline toggles should serialize through the lock.

`group list` should identify group name, enabled state/default, membership paths,
and member counts. Existing `list` is a configuration inventory, so keep disabled
snippets visible and expose enough metadata for later plugin controls. Document
whether counts include nested snippets suppressed by dependencies or profiles.

## Required verification

- Config/schema parity for supported fields; invalid names/paths and duplicates.
- Default state, persistent enable/disable/toggle, restart/load behavior, stale
  overrides, malformed state, atomic-write failure, and concurrent toggles.
- Nested dependency suppression/restoration, app profiles, pack isolation, and
  active duplicate disambiguation through real offline render/CLI fixtures.
- IPC parsing and mutation handler tests without a desktop, including loss of
  response and no duplicate fallback toggle; offline fallback when absent.
- Config reload and group changes clear pending input; failed changes preserve
  last valid matching state.
- README, schemas, compatibility/pack documentation, TASKS, and checkpoint.
- Full fmt/Clippy/tests/help, final diff, local reviewable commit(s). No publish,
  install, or live input. Plugin UI remains a separate later task.
