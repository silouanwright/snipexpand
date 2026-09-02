# Local compatibility suite

This suite exercises SnipExpand's real evdev capture and Wayland injection
pipeline. It must run inside a real Hyprland session, not Docker or ordinary
GitHub Actions.

The fixture covers:

- ASCII punctuation and spacing
- Unicode, accented characters, emoji, and non-Latin text
- Multiline replacement
- Cursor placement followed by more typing
- Immediate Backspace undo
- Twenty consecutive expansions

## Run manually

Build the daemon and driver first:

```bash
cargo build
cargo build --example e2e_type
```

Stop any normal SnipExpand daemon, then start an isolated one:

```bash
XDG_CONFIG_HOME="$PWD/tests/compatibility/config" target/debug/snipexpand
```

In a separate terminal, launch the isolated clean-Neovim target:

```bash
tests/compatibility/launch-target.sh nvim
```

Use `--clean` for the transport baseline. Editor plugins and custom mappings
must be tested separately as application-specific compatibility cases.

With that buffer still focused, run the guarded file verifier from another
terminal:

```bash
tests/compatibility/verify-file.sh
```

The verifier guards focus while the target exists, allows the expected window
closure after the driver sends `:wq`, and compares the saved file byte-for-byte.

Restore the normal user service after the isolated daemon exits.

### Chromium

Close every normal Chromium window, then start the dedicated compatibility
profile:

```bash
tests/compatibility/launch-chromium.sh
```

This persistent profile lives below `~/.cache/snipexpand` by default and loads
the local target with extensions disabled. It is separate from the everyday
profile and does not install 1Password. The launcher warns when another
Chromium process is running because a 1Password prompt could come from that
browser instead.

First leave the browser untouched long enough to confirm that no 1Password
window appears. This launch-only stage distinguishes browser integration from
keyboard injection. Run the typing verifier only as a separate second stage:

```bash
tests/compatibility/verify-clipboard.sh
```

### Other graphical applications

Open one isolated launch-only target:

```bash
tests/compatibility/launch-target.sh zed
tests/compatibility/launch-target.sh electron
tests/compatibility/launch-target.sh gtk
tests/compatibility/launch-target.sh qt
```

The launcher uses a separate profile for Zed and Electron, Zenity's GTK 4 text
view, and the local QML target. It never starts the typing verifier. After the
target is stable, focus its empty text area and run the clipboard verifier as a
separate step.

The verifier runs the same cases, selects the target text, copies it, and
compares the clipboard byte-for-byte with `expected.txt`. `target.html` is the
browser and Electron target; `target.qml` is the Qt target. Do not run the
driver when another window can steal focus. The verifier records the target's
Hyprland address and aborts immediately if focus moves to another window.
The verification copy replaces the current clipboard selection, so preserve
anything important before running it.

## Application matrix

Use the same fixture for each target application. Record the application
version, toolkit, active injection backend, event delay, result, and any timing
override needed.

Current Omarchy test environment: Omarchy 4.0.0-1, Hyprland 0.56.2,
`wtype` 0.4-2, and `wl-clipboard` 2.3.0.

| Application family | Initial targets | Result |
| --- | --- | --- |
| Terminal editor | Neovim 0.12.4 `--clean` in Foot 1.27.0 | Pass: Wayland, 15 ms events, 50 ms and 250 ms post-expansion pauses. Two byte-exact runs passed; the second exposed and corrected a guard false positive caused by the expected `:wq` window closure |
| Configured editor | Personal Neovim profile in Foot | Investigate: first Enter after multiline replacement is consumed by the editor configuration |
| Browser | Chromium 151.0.7922.137, Firefox unavailable | Retest required: isolated Chromium previously passed the complete byte-exact fixture with the superseded wtype compose path. The injector-owned replacement is deterministically covered but has not had a guarded live run. Firefox pending |
| Code editor | Zed 1.14.2 (`zeditor`) | Launch adapter ready; live result pending |
| Electron | Electron 43.4.0 | Launch adapter ready; live result pending |
| GTK | Zenity 4.2.2 / GTK 4.22.4 | Launch adapter ready; live result pending |
| Qt | QML Runtime 6.11.1 | Launch adapter ready; live result pending |

The generic clipboard adapter is available, but the remaining applications
still require controlled live runs. Do not mark a target as passing based only
on visual inspection. Two earlier Chromium attempts were invalid because a
1Password window stole focus. The normal browser with its 1Password extension
was still running, so the prompt cannot be attributed to the isolated test
browser. Those attempts remain discarded; the later focus-stable run above is
the recorded Chromium result.

Chromium 151 converted `🧐` (U+1F9D0) to U+F9D0 when it received the direct
Wayland text keymap. SnipExpand therefore selects Unicode compose for non-BMP
characters only when `non_bmp_input` is `compose`, or when `auto` identifies
a Chromium or Electron runtime. The opt-in `input_method` mode may instead
commit UTF-8 directly when the focused client exposes text-input-v3 and no
other input method owns the Wayland seat. Compose now runs inside the persistent Wayland
injector. It resolves the complete sequence before sending any input, paces the
Ctrl+Shift+U, hexadecimal, and Enter transitions, and clears synthetic
modifiers even when dispatch fails. Compose expansions also use the safer of
the general and compose settle intervals before deleting the trigger. Sequence
resolution, modifier selection, and settle selection have deterministic unit
coverage.

The earlier guarded Chromium 151 run passed the complete byte-exact fixture,
including `🧐`, with no focus change or daemon error. That run exercised the
superseded short-lived `wtype` implementation. Signal subsequently exposed an
intermittent visible `U+1f642` preedit, which motivated the persistent paced
implementation. Chromium and Signal both require new guarded live runs before
the replacement path can be marked compatible.

The first focus-guarded compose run reached Chromium without focus theft but
failed before the emoji because the former wtype implementation passed its
unsupported `-d 0` option. That historical failure is no longer applicable to
the injector-owned implementation.

## Unicode hot reload

To verify that a newly added Unicode character joins the persistent Wayland
keymap without restarting the daemon, start the isolated daemon before copying
`reload/after.yml` into its `match/` directory. Type `;fresh`, save the target,
and compare it byte-for-byte with `reload/expected.txt`. The crab character is
deliberately absent from the startup fixture.

Result: Pass on Hyprland with the Wayland backend. The refreshed keymap handled
the new character directly without invoking the Unicode fallback.
