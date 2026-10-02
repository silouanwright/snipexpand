# Variables and rendering limits

These features are implemented on the authoring branch and are not part of the
published v0.4.1 package yet.

## Reusable echo values

```yaml
global_vars:
  - name: signature
    type: echo
    params: {echo: '{{name}} — Engineering'}
  - name: name
    type: echo
    params: {echo: 'Ada'}
matches:
  - trigger: ';signature'
    replace: '{{signature}}'
    vars:
      - name: name
        type: echo
        params: {echo: 'Zoë'}
```

This renders `Zoë — Engineering`. Globals are scoped to their match file; local
variables override globals by name. `params.echo` is required, may be empty,
and accepts Unicode and multiline text. `echo` never executes a command.

Dependencies in `params.echo` can refer to another echo, a date, a nested-match
variable, or a named capture in that match's regex. SnipExpand resolves them
before their consumers, regardless of declaration order. Cycles and missing
names are configuration errors, even in an unused variable within a snippet.
An unmatched optional regex capture has an empty value. A variable with the
same name as a capture takes precedence.

This is a subset of [Espanso variable injection](https://espanso.org/docs/matches/variables/).
SnipExpand allows forward local references; it does not impose Espanso's serial
local-variable execution constraints. There are no side-effecting extensions.
`depends_on`, `type: global`, dotted fields, and interpolation inside date or
match parameters are unsupported. Date formats/timezones and nested triggers
remain static so `check` can validate them.

## Literal braces and single-pass substitution

Use single-quoted YAML to keep backslashes:

```yaml
matches:
  - trigger: ';literal'
    replace: '{{literal}} | \{\{name}} | {{name}}'
    vars:
      - name: literal
        type: echo
        inject_vars: false
        params: {echo: '{{name}}'}
      - name: name
        type: echo
        params: {echo: 'Ada'}
```

Output: `{{name}} | {{name}} | Ada`.

`\{\{` emits literal opening braces in a replacement or interpolated echo
parameter. Double-quoted YAML needs doubled backslashes. `inject_vars: false`
keeps the entire echo parameter exactly as written, including backslashes.
It is supported only on echo variables.

Only `{{name}}` with letters, numbers, or underscores is a reference. Other
brace expressions remain text. Inserted variable and capture values are never
parsed again. Unresolved names in replacement text remain literal for backwards
compatibility; unresolved names in interpolated echo parameters are errors.
This replaces the old order-sensitive repeated string substitution. To compose
values, declare the reference explicitly inside `params.echo`.

## Bounded rendering

- Templates and string parameters: at most 1 MiB each.
- Variables per snippet: at most 4,096 after merging globals and locals.
- Echo dependency chains and nested-match chains: at most 64 nodes each.
- One expansion: at most 4,096 match/variable evaluations and 1 MiB of cumulative
  generated UTF-8 text, including intermediate values and repeated insertions.

These budgets cover variable/template rendering; typed-case transformation and
cursor handling happen afterwards.

`check` validates graph structure and static limits; runtime and `render` enforce
the cumulative budgets. A large branching graph may therefore pass `check` and
fail `render`. Preview writes no replacement on failure. Automatic expansion
leaves the typed trigger intact and clears its matching buffer. All nested dates
share the same captured instant. Unicode echo text is included when building
Wayland text keymaps; live application delivery still needs the compatibility
matrix checks.

Use `snipexpand render ';signature'` to verify exact output before typing it.
The same rules apply to imported packs; unsupported features are rejected during
inspection/installation. Editor schemas check field shapes; use `check` for
reference resolution and `render` for output budgets.
