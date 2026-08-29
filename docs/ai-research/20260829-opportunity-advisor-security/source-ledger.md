# Source ledger: snippet opportunity advisor security

## Scope fence

Current lane: whether SnipExpand can safely recommend existing snippets based
on text typed manually.

Allowed roots:

- SnipExpand source and documentation
- Public documentation, research, issue trackers, and source repositories for
  input monitoring, text expansion, privacy, and comparable recommendation
  features

Forbidden roots:

- Personal snippet contents beyond the existing configuration schema
- Unrelated files, browser history, clipboard history, or private application
  data on this machine

Out-of-scope fallback rule: do not inspect personal typed-content logs or add
instrumentation during research. Missing evidence remains a documented gap.

| Source | Date | Tier | Relevance |
| --- | --- | --- | --- |
| [Linux kernel input documentation](https://kernel.org/doc/html/latest/input/input.html) | current | 1 | `evdev` delivers raw input events and timestamps directly to userspace clients with device access |
| [TextExpander Snippet Suggestions](https://textexpander.com/learn/using/preferences/snippet-suggestions) | current | 1 | Direct precedent for reminding users when they manually type an existing snippet; supports application allowlists and denylists |
| [TextExpander keystroke and snippet security](https://textexpander.com/learn/accounts/security/how-textexpander-handles-your-keystrokes-keylogging-and-snippet-security) | updated 2025-09-23 | 1 | States that suggestion mode expands its volatile keystroke window from 30 to 300 characters and can observe password fields that applications fail to mark secure |
| [TextExpander usage statistics](https://textexpander.com/learn/accounts/statistics) | current | 1 | Shows the adjacent product value of per-snippet usage and time-saved reports |
| [TextExpander statistics calculation](https://textexpander.com/learn/accounts/statistics/textexpander-statistics-calculated) | current | 1 | Documents the simple characters-saved and WPM calculation |
| [TextExpander local AI recommendations](https://textexpander.com/learn/textexpander-ai-feature-overview-and-faqs/recommendations) | current | 1 | Separate contextual feature runs locally with Gemini Nano and claims not to store keystrokes; broader than the proposed exact-match advisor |
| [Grammarly on keystroke access](https://support.grammarly.com/hc/en-us/articles/360003816032-Is-Grammarly-a-keylogger) | current | 1 | Illustrates disclosure, visible active state, per-context disabling, explicit permission, and best-effort sensitive-field exclusion |
| [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html) | current | 1 | Recommends proportional logging, explicit consent, exclusion of passwords and sensitive personal data, restricted access, and finite retention |
| [NIST Privacy Framework](https://www.nist.gov/privacy-framework) | current | 1 | Frames privacy design as balancing processing benefits against problematic data actions and minimizing data over its lifecycle |
| [LINDDUN privacy threat modeling](https://linddun.org/) | current | 1 | Provides the linkability, identifiability, non-repudiation, detectability, disclosure, unawareness, and non-compliance threat categories used here |
| [Wayland text-input v3 protocol](https://wayland.app/protocols/text-input-unstable-v3) | current | 1 | Defines password and sensitive-data hints for input methods, but these hints are not exposed through SnipExpand's raw `evdev` capture path |
| [Aho-Corasick design notes](https://docs.rs/crate/aho-corasick/latest/source/DESIGN.md) | current | 1 | Confirms exact multi-pattern matching can operate as an automaton over streamed input; implementation details still determine buffering |
| [Historical TextExpander suggestion disclosure](https://jblevins.org/log/suggestions) | 2015-08-21 | 2 | Documents older suggested phrases, including potential passwords, being stored in plaintext and synchronized |
| [TextExpander HIPAA guidance](https://textexpander.com/learn/accounts/security/tips-for-configuring-textexpander-for-hipaa) | current | 1 | Recommends disabling snippet suggestions in a high-sensitivity environment |
| [Sysadmin report of a password suggestion](https://www.reddit.com/r/sysadmin/comments/12wb55l/lets_talk_text_expanders/) | 2023 | 3 | Anecdotal report that a domain-admin password was later offered as a suggestion; useful adversarial evidence, not proof of current behavior |
