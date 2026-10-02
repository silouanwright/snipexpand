# Source ledger

## Scope fence

- Lane: flash-free Unicode replacement through the user's existing Fcitx5 input method.
- Allowed: this repository, installed Fcitx5 5.1 headers and CMake files, official Fcitx5 documentation and source.
- Excluded: live key injection, clipboard access, window focus changes, restarting or stopping Fcitx5, and unrelated repositories.

## Primary sources

- [Fcitx5 input context API](https://github.com/fcitx/fcitx5/blob/master/src/lib/fcitx/inputcontext.h): `commitString`, `deleteSurroundingText`, focus state, and surrounding-text access.
- [Fcitx5 input context implementation](https://github.com/fcitx/fcitx5/blob/master/src/lib/fcitx/inputcontext.cpp): event dispatch semantics for commit and deletion.
- [Fcitx5 developer tutorial](https://fcitx-im.org/wiki/Develop_an_simple_input_method): supported addon structure and factory registration.
- [Fcitx5 basic concepts](https://fcitx-im.org/wiki/Basic_concept): addon, frontend, input context, and event-loop model.
- [Fcitx5 D-Bus module](https://github.com/fcitx/fcitx5/blob/master/src/modules/dbus/dbusmodule.cpp): controller capabilities and session-bus integration.
- [Fcitx5 atomic replacement discussion](https://github.com/fcitx/fcitx5/pull/1621): confirms that ordered deletion and commit events are sufficient in the normal single-threaded UI path; a draft atomic API is not required for this integration.
- Installed Fcitx5 5.1.21 headers under `/usr/include/Fcitx5`: locally authoritative API surface for compilation.
- Installed CMake helpers under `/usr/lib/cmake/Fcitx5Utils`: addon target and compiler conventions.
- [Ubuntu Fcitx5 module development package](https://packages.ubuntu.com/noble/amd64/libdevel/fcitx5-modules-dev): CI package that supplies public module headers in Ubuntu 24.04.
- [Ubuntu Fcitx5 core development files](https://packages.ubuntu.com/noble/amd64/libfcitx5core-dev/filelist): CMake metadata and core headers needed by the standalone addon build.

## Local observations

- Fcitx5 5.1.21 owns the compositor input-method-v2 seat on the target Omarchy system.
- The public controller and virtual-keyboard D-Bus objects do not expose arbitrary text commit.
- `fcitx5-remote` controls activation and state only.
- Fcitx5 exposes a supported in-process addon API with access to the last focused input context.
- Physical evdev observation and Fcitx's client-reported surrounding text arrive
  over different event paths. A stale or temporarily unavailable surrounding
  snapshot is therefore safe to retry once, provided the addon repeats every
  focus, selection, password-field, and exact-suffix check. Signal's ordinary
  composer sets the separate Fcitx `Sensitive` hint, which Fcitx itself uses to
  disable prediction alongside `NoSpellCheck`; it is not equivalent to the
  `Password` capability.
- Ubuntu 24.04 carries Fcitx 5.1.7, which lacks the newer named factory macro
  and exported addon CMake helper present on Omarchy's Fcitx 5.1.21. The
  standard CMake module target and legacy factory entry point work on both.
