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

In a separate terminal, create and focus a disposable Neovim buffer:

```bash
test_file=$(mktemp)
nvim --clean -n "$test_file" -c startinsert
```

Use `--clean` for the transport baseline. Editor plugins and custom mappings
must be tested separately as application-specific compatibility cases.

With that buffer still focused, run the driver from another terminal or a
delayed launcher:

```bash
SNIPEXPAND_E2E_EVENT_DELAY_MS=15 \
  SNIPEXPAND_E2E_EXPANSION_PAUSE_MS=250 \
  target/debug/examples/e2e_type --compatibility
```

The driver writes the cases, saves the file, and exits Neovim. Compare the
result exactly:

```bash
cmp tests/compatibility/expected.txt "$test_file"
```

Restore the normal user service after the isolated daemon exits.

## Application matrix

Use the same fixture for each target application. Record the application
version, toolkit, active injection backend, event delay, result, and any timing
override needed.

| Application family | Initial targets | Result |
| --- | --- | --- |
| Terminal editor | Neovim `--clean` in Foot | Pass: Wayland, 15 ms events, 50 ms post-expansion pause |
| Configured editor | Personal Neovim profile in Foot | Investigate: first Enter after multiline replacement is consumed by the editor configuration |
| Browser | Chromium, Firefox | Pending |
| Code editor | Zed | Pending |
| Electron | Chromium-based desktop app | Pending |
| GTK | Native GTK text editor | Pending |
| Qt | Native Qt text editor | Pending |

Browser and GUI-editor adapters still need a reliable way to extract their
final text for byte-exact comparison. Do not mark a target as passing based
only on visual inspection.
