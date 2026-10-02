# Findings: Optional Wayland Input Method

## Research question

How can SnipExpand use direct UTF-8 insertion when Wayland, the compositor, and
the focused application support it, while preserving virtual-keyboard and
uinput behavior everywhere else?

## Conclusion

Direct UTF-8 commit is useful but cannot be an automatic Omarchy path.
Wayland permits only one input-method-v2 object per seat. Hyprland immediately
sends `unavailable` to a second registrant, and the local Omarchy session's
Fcitx5 instance already owns the role while serving Signal, Chromium, and Foot.

SnipExpand should therefore register input-method-v2 only when a user explicitly
selects `non_bmp_input: input_method`. Default `auto` behavior must remain the
working application-aware split: direct persistent keymaps for terminals and
ordinary characters, paced Unicode compose for Chromium/Electron non-BMP text,
and uinput when Wayland injection is unavailable.

## Protocol lifecycle

- A newly created input-method object starts inactive.
- `activate` and `deactivate` update pending state. The state changes only when
  the following `done` arrives.
- The number of `done` events is the serial required by `commit`.
- Direct replacement can atomically queue `delete_surrounding_text`, then
  `commit_string`, then `commit(serial)`.
- SnipExpand must confirm that the client-reported surrounding text ends with
  the exact trigger and has no active selection before requesting deletion.
  Otherwise a client that does not support surrounding text could append the
  replacement without removing the trigger.
- Requests made while inactive have no effect. SnipExpand must check active
  state before queuing a replacement.
- `unavailable` makes the object inert and is the expected response when an IME
  such as Fcitx5 already owns the seat.

## Failure policy

Before any replacement requests are queued, missing Wayland, missing protocol
globals, seat contention, and inactive text-input-v3 are safe fallback cases.
After a commit is queued, a dispatch error is indeterminate. SnipExpand must not
retry through the keyboard because the application may already contain the
replacement.

## Application reach

Direct commit requires the focused application to expose an active
text-input-v3 context. It is not a terminal replacement path and does not
replace virtual-keyboard-v1 or uinput. Chromium and Electron may additionally
require their Wayland IME flags, although the local Fcitx5 controller currently
reports Wayland contexts for both Chromium and Signal.

## Fcitx5 coexistence

Fcitx5's public controller can activate, deactivate, switch, and configure the
input method, but it does not expose a global method that commits arbitrary text
into whichever application is focused. Its per-context `CommitString` is a
signal back to the client that created that context. A flash-free direct path
through an existing Fcitx5 owner would require a dedicated Fcitx5 integration,
not a second input-method-v2 object.
