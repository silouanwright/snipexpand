# README demo sources

These files provide a synthetic SnipExpand configuration for screenshots and
recordings. They contain no personal snippets.

Run the daemon against the isolated configuration:

```bash
XDG_CONFIG_HOME="$PWD/docs/demo/config" target/release/snipexpand
```

The `e2e_type` example creates a separate uinput keyboard, so scripted demo
typing still travels through SnipExpand's real evdev capture and Wayland
injection pipeline:

```bash
SNIPEXPAND_E2E_EVENT_DELAY_MS=90 \
  SNIPEXPAND_E2E_LABEL_EVENT_DELAY_MS=20 \
  SNIPEXPAND_E2E_NAVIGATION_EVENT_DELAY_MS=120 \
  cargo run --example e2e_type -- \
  --batch-file docs/demo/hero-triggers.txt 2000 --blank-lines --no-final-enter \
  --nvim-conclusion-file docs/demo/hero-conclusion.txt
```

Launch the disposable editor with the normal Neovim theme while disabling
buffer assistance that would alter injected demo text:

```bash
cp docs/demo/initial-hero.txt /tmp/snipexpand-readme-demo.md
foot -w 940x550 -f "Mona Sans Mono:size=14" \
  nvim -n /tmp/snipexpand-readme-demo.md \
    -c "luafile docs/demo/nvim.lua"
```

After a dry run, save the buffer and compare it byte-for-byte with
`expected-hero.txt` before recording a release asset.

Stop the normal user service before recording and restore it afterward. Use a
dedicated workspace containing only synthetic demo content.
