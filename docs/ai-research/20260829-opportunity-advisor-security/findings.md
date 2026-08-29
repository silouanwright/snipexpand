# Findings: snippet opportunity advisor security

## Research question

Can SnipExpand recommend snippets a user should invoke more often without
creating an unacceptable keylogging, retention, or application-boundary risk?

## Preliminary thesis

There is real product precedent and user value, but copying TextExpander's
general phrase-discovery design would enlarge SnipExpand's sensitive input
window and is not justified. The viable design is narrower: detect only exact
manual occurrences of replacements already configured by the user, retain no
arbitrary typed text or hashes, and persist only bounded per-snippet counters
after explicit opt-in.

## Round 1 findings

1. TextExpander explicitly offers reminders when a user types the content of
   an existing snippet, so this is a proven product use case rather than a
   speculative invention.
2. TextExpander's broader suggestion mode keeps a much larger volatile typing
   window. Its documentation also concedes that improperly marked password
   fields can be observed. SnipExpand cannot rely on secure-field metadata,
   particularly inside browsers.
3. SnipExpand already receives raw physical keyboard events to perform its core
   function. The relevant question is therefore the incremental risk from new
   matching state, persistence, reporting, and user expectations, not whether
   the daemon sees keys at all.
4. Contextual AI is unnecessary for the requested feature. Exact matching
   against configured replacement strings can provide the useful reminder
   without sending data away or retaining prose.

## Threat model

### Proposed data flow

1. The existing daemon decodes a physical key event.
2. A matcher advances state only against eligible configured replacement
   strings.
3. A completed exact match emits a snippet identifier.
4. A local store increments a counter for that identifier.
5. The Omarchy panel joins counters with the current configuration to show a
   recommendation.

No arbitrary typed text, rolling plaintext window, discovered phrase, context,
window title, executable name, or clipboard content should cross step 2.

### Material threats

| Threat | Example | Required mitigation |
| --- | --- | --- |
| Disclosure | A state file reveals that a personal-address or medical snippet was manually typed often | Store only opaque snippet IDs and aggregate counts; mode `0600`; no telemetry or cloud sync |
| Detectability and linkability | Per-event timestamps reveal work habits or application usage | Do not store application identity or event timestamps; use one observation start date and aggregate counts |
| Unawareness | A user enables “analytics” without realizing physical typing is evaluated | Default off; explicit plain-language consent; persistent visible enabled state; one-command reset and disable |
| Sensitive-field capture | A browser or terminal password field is invisible to raw `evdev` capture | Never retain unmatched input; reset matcher state on focus changes, shortcuts, navigation, and configured exclusions; disclose that field-level detection is unavailable |
| Mission creep | Exact reminders evolve into arbitrary phrase mining or contextual AI | State the product boundary in code comments, configuration docs, and tests; require a new threat review to cross it |
| Resource abuse | Huge or adversarial replacements inflate matcher memory or CPU | Bound eligible replacement length/count and benchmark configuration loading and per-key processing |

## Architecture assessment

### Reject: general phrase discovery

Discovering new snippets requires retaining or hashing arbitrary sequences,
ranking repeated phrases, and eventually displaying their content. Hashes do
not solve the problem because low-entropy phrases and credentials can be
guessed, and a useful suggestion must ultimately recover plaintext. Historical
TextExpander behavior demonstrates the resulting failure mode.

### Conditionally accept: exact missed-snippet counting

The requested reminder can be implemented without a transcript. Compile
eligible literal replacement strings into a streaming multi-pattern matcher.
Carry only automaton state between characters. On Backspace, navigation,
shortcut modifiers, application changes, or unsupported input, reset the
advisor rather than retaining enough history to reconstruct edits. This may
miss some manually edited occurrences, which is an acceptable privacy-first
tradeoff.

Persist only an opaque snippet identifier and an aggregate missed-use count.
A global observation start date is enough to estimate daily, weekly, monthly,
and yearly savings; detailed time-series or application-level records are not
necessary.

## Existing SnipExpand implications

- The core daemon already retains a bounded plaintext trigger/regex buffer in
  volatile memory. The advisor should not enlarge that buffer to the longest
  replacement.
- SnipExpand cannot determine whether a browser field is a password field from
  its `evdev` stream. Application filtering is useful but cannot be presented
  as field-level protection.
- Debug instrumentation currently includes decoded characters. It is disabled
  by the normal service log level, but character-level logging should be
  removed before adding any advisor state so a debug override cannot turn the
  journal into a typing record.
- Injected SnipExpand output is already excluded from physical input capture,
  preventing ordinary expansions from being miscounted as manual typing.
