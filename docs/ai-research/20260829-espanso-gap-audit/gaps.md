# Espanso gap audit gaps

- Update the Omarchy plugin to include `search_terms` and pass `source` when it
  invokes `paste`, especially for duplicate triggers.
- Live-test profile switching between at least two applications.
- Extend compatibility testing to regex captures, custom word separators, and
  duplicate source selection.
- Reconsider stable snippet IDs only if trigger-based nested references become
  difficult to maintain in real configurations.
