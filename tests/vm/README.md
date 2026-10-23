# SnipExpand desktop tests in Oligarchy

From the repository root:

```bash
python3 tests/vm/run.py
```

The runner builds the current checkout with Cargo, installs its Fcitx bridge in
the guest, and checks exact output in Neovim and Chromium. It also exercises
ASCII, Unicode, and multiline expansion through Oligarchy keyboard input.

The shared runner starts its local PostgreSQL database and the pinned Oligarchy
server, reserves a slot, boots a disposable Omarchy disk overlay, copies the
explicit project payload, runs the suite, saves evidence, and stops the services.
A failed check returns a nonzero exit status. No host desktop fallback is allowed.
The suite refuses to use occupied ports or an already-running harness session.

## Local prerequisite

This machine has the prepared harness at
`~/Documents/ChatGPT/SnipExpand/oligarchy-pilot`. Override its location with
`OLIGARCHY_HARNESS_HOME`. The wrapper checks for that installation and refuses
if it is missing; it does not download software or provision a VM implicitly.
See that directory's `AUTOMATION.md` for setup, ports, artifacts, and limitations.
This is an on-demand local VM test command, not a hosted CI job or scheduled task.

Each run records the source commit, dirty status, and SHA-256 of every payload
file. Uncommitted changes in the selected payload are tested too. Screenshots,
project results, Oligarchy action history, and exported guest logs live under the
harness's `artifacts/<timestamp>/snipexpand/`.
No credentials or disk images belong in this repository.

`tests/vm/suite.py` owns the project assertions. The shared runner owns lifecycle
and cleanup. The guest marker and virtualization checks must run before any
service, display, keyboard, clipboard, plugin installation, or state operation.
