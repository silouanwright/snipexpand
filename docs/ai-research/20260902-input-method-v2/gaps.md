# Gaps: Optional Wayland Input Method

- Guarded live validation remains intentionally blocked until the user gives
  fresh approval. No direct-commit input or service restart occurred here.
- Confirm direct replacement against a text-input-v3 client in an environment
  where Fcitx5 is intentionally absent. This must not be tested by stopping the
  user's active Fcitx5 session without separate approval.
- Verify behavior on a compositor other than Hyprland before describing the
  protocol mode as generally supported.
- A future Fcitx5-native integration would need a separately designed addon or
  supported upstream API. The current public D-Bus controller cannot perform
  arbitrary focused-client commits.
