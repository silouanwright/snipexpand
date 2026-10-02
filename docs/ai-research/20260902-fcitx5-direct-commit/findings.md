# Findings

## Recommended design

Ship a small Fcitx5 addon with SnipExpand. The addon registers a narrow object
on Fcitx5's existing per-user D-Bus service and accepts a replace request
containing the exact trigger and its replacement. Inside Fcitx5, it verifies
that the focused input context has valid surrounding text, no selection, and
the exact trigger immediately before the caret. Only then does it delete the
trigger and commit the final UTF-8 replacement.

This is preferable to acquiring `zwp_input_method_manager_v2` directly because Fcitx5 already owns that exclusive Wayland role on Omarchy. It also avoids Unicode-compose preedit, which is the visible `U+1f642` flash in Chromium and Electron applications.

## Safety properties

- The addon acts only on Fcitx5's currently focused context.
- The exact expected trigger must be present directly before the caret.
- Active selections, invalid UTF-8, and oversized requests are refused.
- Input contexts marked as password fields by Fcitx5 are refused. Fcitx's
  broader `Sensitive` hint is not a password indicator; Signal sets it on its
  ordinary private-message composer to disable prediction. It is allowed by
  default and can be suppressed globally or per application profile with
  `fcitx_sensitive_hint: suppress`.
- A stale surrounding-text snapshot gets one 15-millisecond exact retry.
- A client that never provides surrounding text gets one guarded fallback.
  The first refusal creates a 100-millisecond permit bound to the same focused
  input context, trigger, and replacement. The second call forwards only the
  trigger's Backspaces and commits final UTF-8 through that same Fcitx context.
- A suffix mismatch never authorizes blind deletion. Changed focus or request,
  selection, password fields, and expired permits are refused.
- A refused request is safe to route to the existing compose fallback.
- An indeterminate transport failure must not be retried through another injector because the first request may already have committed.
- Terminals retain SnipExpand's direct modifier-free Wayland keymap path.

## Packaging

Fcitx5 supports per-user addon metadata under its user data directory. The
addon metadata can point to an absolute shared-library path, so installation
does not require writing into system Fcitx directories. The installer asks a
running Fcitx5 process to refresh its addon list without stopping it. That code
is covered offline but has not been invoked during this noninteractive pass.

The source is embedded in the Rust binary and compiled during setup when Fcitx5
development headers and a C++ toolchain are available. Failure to build or load
the optional bridge leaves the existing compose fallback operational. The
standalone CMake target deliberately uses standard `add_library(MODULE)` and
the legacy-compatible factory symbol so it builds with Fcitx 5.1.7 through
5.1.21.
