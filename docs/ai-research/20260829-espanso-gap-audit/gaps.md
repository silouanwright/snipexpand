# Espanso gap audit gaps

- Verify whether the Omarchy plugin already searches replacement text and how
  naturally `search_terms` can enter its current model.
- Decide whether nested matches should reference a trigger or a future stable
  snippet ID. Espanso references triggers, which is compatible but couples
  composition to user-facing abbreviations.
- Test word-boundary behavior with non-US layouts before choosing default
  separator semantics.
- Revisit duplicate-trigger selection only after real users ask for multiple
  snippets behind one abbreviation.
