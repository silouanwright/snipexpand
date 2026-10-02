# Findings: Chromium/Electron Non-BMP Input

## Research question

Why do characters above `U+FFFF`, such as 🙂 (`U+1F642`), become private-use glyphs in Chromium/Electron when injected through SnipExpand's custom Wayland XKB keymap, and how do other projects avoid the problem?

## Observed behavior

- Signal rendered 🙂 as a bar-like private-use glyph.
- After enabling the compose fallback, the user reports that the expansion is
  also intermittent in Signal and sometimes does not happen.
- The output corresponds to truncating `U+1F642` to `U+F642`.
- Chromium previously converted 🧐 (`U+1F9D0`) to `U+F9D0` through the same direct-keymap path.
- SnipExpand's current local fix detects Chromium/Electron applications and uses synchronized Unicode compose only for non-BMP characters.

## Current thesis

There are two separate failures.

### 1. Direct XKB injection is truncated inside Chromium

The Wayland compositor and xkbcommon are capable of carrying a non-BMP
character. Chromium's XKB layout engine initially retrieves the character as a
32-bit value and stores it in a `DomKey`. The later legacy
`KeyEvent::GetCharacter()` API explicitly supports only BMP characters. Its
implementation casts the 32-bit value to `char16_t`.

That gives an exact explanation for the bar-like output:

```text
🙂  U+1F642
       │
       └─ narrowed to 16 bits by Chromium
          U+F642, a private-use character
```

The debug build contains an assertion against this narrowing, but a normal
release build returns the truncated value. This is a Chromium event-model
limitation rather than an invalid XKB keysym. Electron applications inherit the
same Chromium input stack.

The Element Desktop report in bemoji issue 34 is an independent reproduction
of the same family of bug: emoji including 👍 and 🤣 become private-use
characters when typed through wtype.

### 2. The compose workaround is not committing reliably

The screenshot containing an underlined `U+1f642` is not the truncation bug.
It is numeric Unicode preedit. It proves all of the following happened:

1. SnipExpand matched the trigger.
2. Signal received Ctrl+Shift+U and entered Unicode input mode.
3. Signal received the hexadecimal digits `1f642`.
4. The sequence did not complete its final commit.

GTK documents Enter as the final step of the sequence and exposes separate
preedit and commit phases. The safest conclusion is that the final Return was
not received or processed at the required point. SnipExpand currently launches
a short-lived wtype process for each non-BMP character. With the default
Wayland injection delay of zero, the generated sequence has no configurable
pause before the commit key. A Wayland roundtrip confirms that protocol
requests were handled by the compositor; it does not acknowledge that Signal's
text widget has applied each input-method state transition.

wtype itself contains fixed internal sleeps and roundtrips, but its issue
tracker still contains Chromium-specific character loss, raw-keycode
misinterpretation, modifier-release failures, and context-dependent results.
That matches the user's description of this failure as spotty.

## How other projects handle it

### wtype

wtype creates a custom XKB keymap and emits raw virtual-keyboard events. Its
open Chromium reports show that this route is not dependable across all
characters. It does not have an application acknowledgment that confirms text
was inserted.

### bemoji

bemoji explicitly documents wrong emoji in Chromium/Electron applications. It
offers two practical workarounds:

- copy the character and synthesize paste;
- use `wl-ime-type`, which inserts text through Wayland input-method-v2.

The second option avoids fake character key events, but it works only when the
focused application supports text-input-v3. Chromium/Electron support may also
need command-line flags. Terminals are not uniformly supported.

### Espanso

Espanso supports both injection and clipboard-based insertion. Its schema
exposes several timing knobs, including modifier, key, text-injection, and
paste-shortcut delays. This is evidence that target applications miss events
when injection runs too quickly. It is not a universal Wayland solution, and
clipboard fallback brings privacy and clipboard-restoration tradeoffs.

### Wayland input-method-v2

The protocol's `commit_string` operation sends a UTF-8 string for direct
insertion. It therefore avoids Chromium's legacy `char16_t` keyboard-event
path and avoids a visible Ctrl+Shift+U preedit sequence. It also supports
deleting surrounding text, which could eventually replace synthesized
Backspace for a full expansion.

This is the cleanest architecture where it is available, but it cannot be the
only backend:

- the target must participate through text-input;
- support varies, particularly in terminals;
- only one input-method object is allowed per seat, so SnipExpand must coexist
  safely with real IMEs.

## Recommendation

### Short term

Keep SnipExpand's direct persistent Wayland keymaps for terminals and ordinary
characters. For Chromium/Electron non-BMP characters only, replace the current
zero-delay, short-lived compose command with a deliberately paced compose state
machine:

1. finish trigger deletion;
2. wait a small compose-specific settle interval;
3. press and release Ctrl+Shift+U with explicit modifier cleanup;
4. send the hexadecimal digits with a small inter-key delay;
5. wait again before Return;
6. release all state and finish the Wayland roundtrip.

This should be implemented inside the persistent injector if practical. It
removes process startup and object-destruction variance and gives SnipExpand
control over every transition. Compose timing should have its own setting so
users do not have to slow the fast direct-keymap path. Deterministic tests can
verify ordering, releases, app routing, and delay selection, although one
guarded Signal test will still be needed to validate the application boundary.

Do not send a second commit key as a blind retry. If the first one succeeded,
the second can insert a newline or activate UI. Do not make clipboard insertion
the default because snippets can contain sensitive data and users expect the
clipboard to remain untouched.

### Medium term

Prototype an optional input-method-v2 backend that uses direct `commit_string`
for applications advertising text-input-v3. Keep the current persistent
virtual-keyboard backend as the compatibility path. This gives SnipExpand the
best available path per application instead of forcing one mechanism across
terminals, GTK/Qt applications, and Chromium/Electron.

### Upstream

File a focused Chromium issue with the direct-keymap reproduction and source
trace. The narrow `char16_t` API is explicit and longstanding, so an upstream
fix is likely broader than removing one cast. The current search did not locate
an existing Chromium issue that precisely tracks non-BMP custom-XKB input.

## Confidence

- Direct-keymap truncation root cause: high. The observed transformation exactly
  matches Chromium source.
- Visible `U+1f642` interpretation: high. It is the documented numeric Unicode
  preedit sequence before commit.
- Exact reason Signal sometimes misses the commit: medium. The boundary lacks an
  acknowledgment, but current behavior and peer-project evidence strongly point
  to sequencing/timing rather than matching or app detection.
