# Gaps: Chromium/Electron Non-BMP Input

- No exact upstream Chromium issue was located for non-BMP input through a
  custom Wayland XKB keymap. The source-level limitation is confirmed, but the
  existing Chromium bug identity remains unknown.
- Electron has no separate text injection layer identified in this review.
  Signal and Element behavior is consistent with inherited Chromium handling.
- The visible `U+1f642` proves matching, app routing, compose activation, and hex
  entry succeeded. It cannot distinguish a lost Return event from Signal
  processing Return before its preedit state was ready.
- Wayland virtual-keyboard provides no application-level acknowledgment of
  committed text. Deterministic unit tests cannot prove Signal accepted the
  result; a guarded live test remains necessary after a timing/state fix.
- Input-method-v2 viability on this exact Hyprland and Signal configuration has
  not been tested. Doing so would require an explicitly approved live desktop
  test and careful coexistence checks with any active IME.
- The minimum reliable compose delay is target- and machine-dependent. It must
  be bounded through a small matrix rather than inferred from one successful
  attempt.
