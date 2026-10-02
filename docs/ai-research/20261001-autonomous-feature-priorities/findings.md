# Which SnipExpand work is worth doing next?

Research date: 2026-10-01. Source IDs refer to [the ledger](source-ledger.md).

## Recommendation

Start with stronger configuration checking, offline expansion preview, and
editor schemas. Then add timezone-aware dates and a carefully bounded echo
variable. These improve ordinary snippet authoring and let future work be
verified without repeated live desktop tests. This ranking is an engineering
judgment based on the user's preference for low involvement, not a popularity
ranking or a promise of compatibility with every application.

The earlier brainstorm ranked random/UUID too highly and treated locales as
part of a small date enhancement. Research separates those into later work.

## Recommended sequence

| Priority | Work | User benefit | Effort / user involvement |
| --- | --- | --- | --- |
| 1 | Stronger `check` plus offline rendered preview | Catch bad snippets before typing them; inspect actual expanded text, source, and cursor position | Medium / automated fixtures and CLI tests |
| 2 | SnipExpand YAML schemas and editor setup | Completion and immediate errors while editing matches and settings | Small–medium / isolated schema validation |
| 3 | Date validation and explicit timezone support | UTC timestamps and named-zone dates regardless of desktop timezone | Small for hardening; medium for zones / fixed-clock tests |
| 4 | Bounded `echo` compatibility | Reuse a name, address, or signature and accept more existing Espanso YAML | Small for literals; medium if dependency interpolation included / renderer tests |
| 5 | Local snippet groups | Switch a personal work/language/project collection on or off | Medium–large / core automated; plugin integration needs UI checks |

### 1. Check and preview: strongest first investment

There is no render CLI, but internal literal-trigger rendering already exists
(L2, L3). Exposing it would help authors and make subsequent feature tests more
useful. Current plugin previews show stored text, not evaluated variables (L4).

Proposed first slice: `snipexpand render TRIGGER --source FILE`, optional JSON,
no daemon or input permission requirement. Report ambiguity instead of choosing
an arbitrary duplicate. Define explicit profile selection; do not depend on
whatever window happens to be focused. Regex sample-input evaluation can follow
rather than inflating the first command. Avoid configuration creation as a side
effect of preview (current main calls ensure_config before dispatch).

Validation has a concrete gap: `Config::validate` does not check date format
strings or offset bounds, while `render` directly formats and adds the offset
(L1, L2). This is a code-inspection risk, not a reproduced crash in this research.
Add invalid-format and extreme-offset fixtures and make rendering return useful
errors. Check irrelevant parameters by variable kind too. Do not automatically
reject all unknown double-brace text: code templates may intentionally contain
literal placeholders, and Espanso defines escaping behavior (S2).

Acceptance: exact output and cursor metadata, duplicates, nested variables,
invalid dates, and render failures are tested without input injection. Success
here proves rendering, not application delivery.

### 2. Editor schemas: a useful new candidate

Espanso maintains a schema, with a direct user request explaining its validation
benefit (S3, S4). No equivalent schema was found locally (L1). A SnipExpand schema
would expose its actual supported keys in YAML-capable editors, reducing trial
and error. Do not reuse Espanso's full schema: it would suggest unsupported keys.

Acceptance: settings and match schemas, editor setup examples, and valid/invalid
fixtures checked against both schemas and the Rust parser. Cross-file cycles
and profile-dependent conflicts remain the CLI validator's responsibility.
Schema/parser drift is the maintenance cost; automate agreement on fixtures.

### 3. Dates: prioritize timezone support over full locale support

Local dates and second offsets already exist (L1, L2). Two upstream requests
specifically ask for UTC timestamps independent of the machine timezone (S5,
S6), and Espanso now documents `tz` (S1). This is a clearer use case than random
greetings. Use `tz` naming for compatibility, validating unknown zones explicitly;
if rejecting invalid zones rather than silently falling back, document that
intentional difference from Espanso.

UTC can use current Chrono; full IANA names need timezone data, such as Chrono-TZ
(S11, S12). Test one captured instant per expansion, UTC, named zones, DST,
format errors, and checked offsets. Seconds offsets are elapsed time, not a
promise of the same wall-clock time tomorrow across DST. Locale support is
separate: Chrono's locale API is marked unstable (S11), and mapping Espanso's
locale names needs deliberate compatibility work. Defer it unless needed.

### 4. Echo: useful compatibility, but define the boundary

Echo enables shared constants without adding a trigger for each constant (S1).
Local global-variable merging and nested matches already exist, so this adds
less new expressive power than it first appears (L1, L5). Its strongest reason
is accepting existing snippets with less rewriting; no pack-adoption increase
was measured in this research.

Full Espanso variable behavior includes interpolation inside parameters,
dependency ordering, and escaping (S2). Current rendering is ordered string
replacement (L2). A literal echo implementation must not be advertised as full
variable compatibility. Either support a tested dependency subset or reject
unsupported semantics clearly; never silently produce unresolved substitutions.

### 5. Groups: useful larger follow-up, not the easiest first win

There is an upstream organization use case (S8). However, file organization,
app profiles, and pack enable/disable already cover portions of it (L3, S7).
The missing feature is runtime control of personal collections. Specify how
group state combines with profiles, packs, nested references, and duplicate
triggers. Persist state and test reload behavior. Start with CLI/config/IPC;
plugin controls follow. No evidence here justifies delaying all other work for
this larger design.

## Reasonable later, with a specific use case

- **Random choices:** supported upstream (S1), easy to test with controlled RNG,
  but no strong demand signal found in this round. Optional small addition;
  below authoring and reliability work.
- **Native UUID:** the Python-backed Hub package demonstrates a developer use
  case (S9). Nice standalone addition, but it will not automatically enable that
  script-based package. Prefer it when a user actually needs generated IDs.
- **Explicit Copy in the plugin:** practical follow-up to rendered preview.
  Upstream requests it as an alternative to direct insertion (S10). This would
  intentionally replace clipboard content when clicked, unlike an automatic
  clipboard injection backend. Core rendering is testable offline; end-to-end
  clipboard/UI behavior still needs a scoped test.
- **AUR packaging and clean-install CI:** useful distribution/reliability work
  already in P0. Packaging checks can be automated, but publication requires
  the appropriate account and active-seat permission behavior needs an actual
  session test. Do not promise wholly unattended end-to-end validation.

## Poor fits for this particular next round

Forms/choice popups, multiple Tab stops, clipboard-based automatic insertion,
and more injection backends introduce focus, cursor, clipboard, or application
behavior that synthetic renderer tests cannot settle (S14; local architecture).
They conflict with the current goal of needing little live desktop involvement.
They are not inherently bad features.

Shell/script execution would also add timeouts, failure handling, and trust
policy to the core; it remains an explicit product non-goal (L5). Full personal
Espanso migration remains premature while the supported format is changing.
A read-only compatibility audit could be considered separately, but pack
inspection already exists and should not be rebuilt under another name (L3).

## What is already done

Search labels/terms, nested matches, pause controls, app profiles, regex,
word boundaries, duplicate-source selection, and Git/Hub pack management are
already implemented. Avoid repackaging these as new proposals (L3, L5).
