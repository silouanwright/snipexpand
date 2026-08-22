---
name: snipexpand-shortcuts
description: Add, remove, inspect, validate, and organize SnipExpand text-expansion shortcuts.
version: 2.0.0
---

# SnipExpand shortcuts

Use this skill when the user asks to manage SnipExpand shortcuts, expansions,
match files, trigger behavior, or application exclusions.

## Current interface

- CLI: `snipexpand`
- Settings: `~/.config/snipexpand/config.yml`
- Handwritten matches: `~/.config/snipexpand/match/**/*.yml` or `*.yaml`
- CLI-managed matches: `~/.config/snipexpand/match/generated.yml`
- Service: `snipexpand.service`

## Safety and editing rules

1. Read the current state with `snipexpand list` and validate it with
   `snipexpand check` before changing anything.
2. Use `snipexpand add` or `snipexpand remove` for simple static expansions. These
   commands modify only `match/generated.yml` and automatically notify a
   running daemon.
3. Edit a handwritten YAML file for advanced behavior such as multiple
   triggers, boundaries, cursor positioning, variables, or case propagation.
4. Preserve comments, organization, and unrelated user entries. Never rewrite
   every match file merely to change one shortcut.
5. After a CLI change, run `snipexpand check` and `snipexpand list`. An explicit
   `snipexpand reload` is unnecessary unless automatic notification failed.
6. After a manual file edit, run `snipexpand check`. Match files are watched and
   normally reload automatically; use `snipexpand reload` when an immediate,
   explicit reload is useful.
7. If validation fails, report the exact error and do not leave a knowingly
   invalid configuration in place.
8. Do not add shell commands, scripts, forms, regex triggers, imports, or other
   unsupported Espanso fields. SnipExpand rejects unknown fields.

## Simple CLI operations

List and validate:

```bash
snipexpand check
snipexpand list
```

Add or replace a generated expansion:

```bash
snipexpand add ';mail' 'user@example.com'
```

Use the literal two-character sequence `\n` for newlines in a CLI argument:

```bash
snipexpand add ';sig' 'Best regards,\nSilouan'
```

Remove a trigger from `generated.yml`:

```bash
snipexpand remove ';mail'
```

`remove` does not delete triggers defined in handwritten files. Use the source
path shown by `snipexpand list` to find and edit those entries.

## Advanced YAML matches

Use a focused file such as `~/.config/snipexpand/match/personal.yml`:

```yaml
global_vars:
  - name: today
    type: date
    params:
      format: "%Y-%m-%d"

matches:
  - triggers: [";mail", ";email"]
    replace: "user@example.com"

  - trigger: ";sig"
    word: true
    replace: |
      Best regards,
      Silouan

  - trigger: ";function"
    replace: |
      fn example() {
          $|$
      }

  - trigger: ";today"
    replace: "{{today}}"

  - trigger: ";hello"
    propagate_case: true
    uppercase_style: capitalize_words
    replace: "good morning"
```

Supported match fields:

- `trigger` or `triggers`
- `replace`
- `word`, `left_word`, and `right_word`
- `propagate_case`
- `uppercase_style`: `uppercase`, `capitalize`, or `capitalize_words`
- match-local `vars` and file-level `global_vars` of type `date`
- date `params.format` and signed `params.offset` in seconds

The first `$|$` in a replacement sets the final cursor position.

## Trigger behavior

Read `~/.config/snipexpand/config.yml` before assuming how a shortcut fires:

```yaml
trigger_mode: space       # or immediate
terminators: [space]      # space, enter, and/or tab
```

- `immediate` expands when the trigger is complete unless a right boundary is
  required.
- `space` waits for a configured terminator and removes that terminator with
  the trigger.
- `word` requires both left and right word boundaries.
- `left_word` and `right_word` require only their corresponding boundary.
- Duplicate triggers are invalid. Prefix-related triggers are supported and
  resolved deterministically; do not claim they are forbidden.

## Application exclusions

Focus the application and inspect its properties:

```bash
snipexpand detect
```

Then add a regex filter to `~/.config/snipexpand/config.yml`:

```yaml
app_exclusions:
  - class: "^1Password$"
  - title: "Secret"
    exec: "/vault$"
```

Fields within one entry are combined with AND. Separate entries are combined
with OR. Exclusions stop expansion but do not stop the daemon from observing
kernel input events.

## Troubleshooting

```bash
snipexpand doctor
snipexpand status
snipexpand status --json
systemctl --user status snipexpand
journalctl --user -u snipexpand -n 100 --no-pager
```

`snipexpand status` contacts the daemon instead of trusting that a socket file
exists. Its JSON form is suitable for status bars and desktop integrations and
includes configuration validity, version, process ID, and loaded match counts.
It also reports the active `wayland` or `uinput` injection backend.
In immediate mode, it warns when a shorter trigger makes a longer trigger
unreachable and names both source files.

If Espanso is running, it may exclusively grab the keyboard and prevent SnipExpand
from seeing events. Do not disable services or change group membership unless
the user explicitly authorizes that system change.

## Completion report

Tell the user:

- which trigger or setting changed;
- which file owns it;
- whether `snipexpand check` passed;
- whether the running daemon reloaded, when relevant;
- any limitation that changes the requested behavior.

## Installing this skill

Place this file where the target coding assistant discovers personal skills.
For Codex, a personal installation can be created with:

```bash
mkdir -p ~/.codex/skills/snipexpand-shortcuts
cp skills/snipexpand-shortcuts.md ~/.codex/skills/snipexpand-shortcuts/SKILL.md
```

Other agents can use the same Markdown instructions from their own skill or
instruction directory.
