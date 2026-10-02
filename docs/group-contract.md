# Personal groups — implementation contract

Implemented on `codex/snippet-authoring`; not published in v0.4.1. These controls
manage personal collections through configuration, CLI, and IPC. Plugin buttons
remain a separate follow-up.

## Configuration and selection

Add to `config.yml`:

```yaml
snippet_groups:
  - name: work
    match_files: [work, signatures.yml]
    enabled: true
  - name: greek
    match_files: [languages/greek.yml]
    enabled: false
```

Names are unique, case-sensitive ASCII identifiers using letters, numbers, `_`,
and `-`. Paths are
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
references still fail validation, including in disabled groups. Profile-excluded
references now follow the same suppression rule; earlier versions rejected the
entire configuration in this situation.

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
- The CLI uses corresponding daemon IPC operations while running. If no daemon can be
  connected, apply the same persistence transition offline.
- Never retry a toggle offline after a request may have reached the daemon:
  a lost response must not cause a second toggle.
- JSON IPC uses an explicit operation and name, with structured response data;
  handle errors without terminating the daemon.

Commands keep user YAML and comments intact. Overrides are stored in a separate versioned
`groups.json` under the configuration directory, with atomic same-directory
replacement and an exclusive lock around read/modify/write. Defaults come from
`config.yml`; an override wins. To return to a YAML default, remove that name
from `groups.json` while the daemon is stopped, or set the same value explicitly. Ignore unknown saved names for selection but
preserve them so a temporarily removed definition can recover its prior state.
Invalid state files fail validation instead of silently resetting preferences.

Load and validate a candidate before committing an override. If persistence
fails, the running matcher and previous state file must remain unchanged. After
persistence succeeds, install that exact candidate in memory; do not re-read a
different candidate. Cancel partial input, pending expansion, and undo on a
successful transition. The recent reload helper provides a seam for these tests.
Concurrent offline toggles should serialize through the lock.

`group list --json` returns `name`, `enabled`, `default_enabled`, `overridden`,
`match_files`, `members`, and `available`. `members` counts configured match
definitions in that group (not trigger aliases). `available` counts members left
after all group and dependency filters, without an app profile. It does not
promise that a trigger is unambiguous or reachable, or that expansion is globally
enabled. While running, listing reports the daemon's loaded state; otherwise it
loads disk configuration. A failed config reload leaves the daemon listing its
last valid state; `check` diagnoses the disk error.

`list --json` remains a complete inventory, including disabled snippets. It adds
`groups` and `available` with the same no-profile selection rule. Existing status
`match_groups` counts snippet definitions, not these named personal collections.

Example commands:

```sh
snipexpand check
snipexpand group list --json
snipexpand group disable work
snipexpand group enable greek
snipexpand group toggle work
snipexpand render ';signature'
```

Group commands do not create starter YAML or install/update the bundled skill.
An unknown group is an error. A daemon too old to support group IPC returns an
error; the client does not fall back after connecting. Upgrade/restart it before
using these commands. A timeout or lost response may mean the change committed:
inspect `group list` before repeating a toggle.

Wire protocol (one JSON request/response per line):

```text
group<TAB>{"operation":"toggle","name":"work"}
{"status":"ok","groups":[...]}
```

Other operations are `list` (no name), `enable`, and `disable`. Failures use
`{"status":"error","error":"message"}`. The settings and override file are
SnipExpand-specific, not Espanso configuration keys.

## Verification coverage

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
- Full fmt/Clippy/tests/help, package verification, and final diff. No publish,
  install, or live input. See the completion audit for command results.
