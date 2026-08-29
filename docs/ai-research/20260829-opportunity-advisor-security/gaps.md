# Gaps: snippet opportunity advisor security

- Decide whether eligibility should be global after opt-in or explicit per
  snippet. Per-snippet consent is safer but creates substantial setup friction.
- Benchmark a streaming exact matcher against configurations with thousands of
  snippets and unusually long replacements.
- Decide whether the panel should show only one recommendation at a time or a
  ranked list. Avoid desktop notifications that expose snippet metadata.
- Define how renamed, moved, duplicated, dynamic, multiline, and
  cursor-positioned snippets map to stable opaque identifiers.
- Determine whether enterprise or shared-machine use should disable the feature
  entirely rather than offer application filtering.
