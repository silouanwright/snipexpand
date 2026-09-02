# Source Ledger: Chromium/Electron Non-BMP Input

## Scope Fence

Current lane: non-BMP Unicode injection through Wayland virtual keyboards into Chromium and Electron applications.

Allowed roots:

- `/home/silouan/Work/snipexpand`
- Upstream Chromium, Electron, Wayland, xkbcommon, wtype, Espanso, and relevant text-expander repositories and issue trackers

Forbidden roots:

- Unrelated local repositories and user data
- Live desktop input, clipboard, or application automation

Out-of-scope fallback rule: if evidence cannot be found in the allowed sources, record the gap instead of broadening the search or running a live test.

| Source | URL/path | Date | Tier | Relevance |
| --- | --- | --- | --- | --- |
| Chromium `KeyEvent` API | https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/events/event.h | accessed 2026-09-02 | Tier 1 | Documents that `GetCharacter()` supports only BMP characters and returns `char16_t`. |
| Chromium `KeyEvent::GetCharacter()` | https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/events/event.cc | accessed 2026-09-02 | Tier 1 | Shows the `uint32_t`/`DomKey` character being cast to `char16_t`, which truncates non-BMP code points in release builds. |
| Chromium XKB layout engine | https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/events/ozone/layout/xkb/xkb_keyboard_layout_engine.cc | accessed 2026-09-02 | Tier 1 | Shows Chromium receiving XKB text as a 32-bit value before the later `KeyEvent` narrowing. |
| xkbcommon keysym conversion | https://github.com/xkbcommon/libxkbcommon/blob/master/src/keysym.c | accessed 2026-09-02 | Tier 1 | Confirms XKB Unicode keysyms can represent scalar values through `U+10FFFF`; the source limitation is not XKB itself. |
| Wayland virtual keyboard protocol | https://wayland.app/protocols/virtual-keyboard-unstable-v1 | accessed 2026-09-02 | Tier 1 | Defines raw keymap, key, and modifier requests. It does not provide direct UTF-8 text commit. |
| Wayland input method v2 protocol | https://wayland.app/protocols/input-method-unstable-v2 | accessed 2026-09-02 | Tier 1 | Defines `commit_string` and `delete_surrounding_text`, the clean direct-text alternative, plus the one-input-method-per-seat restriction. |
| GTK numeric Unicode input | https://docs.gtk.org/gtk4/class.IMContextSimple.html | accessed 2026-09-02 | Tier 1 | Documents Ctrl+Shift+U, hexadecimal digits, then Enter, and distinguishes preedit from commit. |
| wtype issue 31 | https://github.com/atx/wtype/issues/31 | 2021-09-22 | Tier 1 | Longstanding open report that Chromium accepts some custom-keymap characters and rejects others. |
| wtype issue 66 | https://github.com/atx/wtype/issues/66 | accessed 2026-09-02 | Tier 1 | Reports that destroying a virtual keyboard does not always clear modifier state reliably. |
| wtype issue 71 | https://github.com/atx/wtype/issues/71 | 2025-12-11 | Tier 1 | Reproduces Chromium/Electron interpreting a dynamically assigned keycode as Backspace. |
| wtype issue 72 | https://github.com/atx/wtype/issues/72 | accessed 2026-09-02 | Tier 1 | Reports application- and context-dependent character failures in Chromium-derived applications. |
| wtype issue 73 | https://github.com/atx/wtype/issues/73 | accessed 2026-09-02 | Tier 1 | Documents wtype's fixed per-key sleeps even when its configurable delay is zero. |
| wtype source | `/tmp/snipexpand-research-wtype` at `d71be3a7b3f93b534a2823fd68cabd7ac2a02359` | accessed 2026-09-02 | Tier 1 | Confirms dynamic keycode assignment, per-key roundtrips, modifier requests, and internal sleeps. |
| bemoji issue 34 | https://github.com/marty-oehme/bemoji/issues/34 | 2024-07-08 | Tier 1 | Exact Electron-family symptom: emoji become private-use glyphs in Element Desktop. |
| bemoji README known issues | https://github.com/marty-oehme/bemoji#known-issues | accessed 2026-09-02 | Tier 1 | Documents clipboard and `wl-ime-type` workarounds, text-input-v3 limitations, and Chromium flags. |
| Espanso configuration schema | https://github.com/espanso/espanso/blob/dev/schemas/config.schema.json | accessed 2026-09-02 | Tier 1 | Documents injection, modifier, key, paste, and clipboard timing controls used to cope with target-specific misses. |
| Current SnipExpand implementation | `src/app.rs`, `src/daemon.rs`, `src/injector.rs`, `src/config.rs` | 2026-09-02 | Tier 1 | Establishes app detection, direct keymap path, short-lived wtype compose fallback, and current timing knobs. |
