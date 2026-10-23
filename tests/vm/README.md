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

## Firefox and Zed campaign

```bash
SNIPEXPAND_VM_CAMPAIGN=firefox-zed \
  /home/silouan/Work/projects/synaxis/scripts/synaxis-build-budget.sh -- \
  python3 tests/vm/run.py
```

This optional campaign uses the same Oligarchy lifecycle and disposable overlay.
The default Neovim/Chromium suite remains available without the environment flag.
The shared build limiter serializes heavy work on this machine; the Oligarchy
runner separately locks its VM slot. The command above uses both.

Missing Firefox/Zed packages are installed only inside the guarded guest, with a
full system-package upgrade through `omarchy update system-pkgs`, followed by
`omarchy pkg add`, to avoid a partial upgrade. This updates packages only; it is
not a full Omarchy migration or a test of the new kernel after reboot. Network/package failures
are setup failures, not application compatibility verdicts. Package versions
and installation output are retained in the run evidence.

Each application gets a separate profile and daemon configuration. The fixture
compares clipboard bytes for ASCII, Unicode including non-BMP emoji, multiline
text, cursor placement, Backspace undo, and twenty repeated expansions. It then
adds, changes, and removes a Unicode snippet through reload, checking each output
and that the daemon remains alive. All text is synthetic. Focus guards abort
input if another guest window takes focus. `--probe-copy` is a test-driver mode
used by this guarded guest runner; never invoke input drivers on the host.

Each app has its own result JSON. A mismatch does not skip the other app; missing
applications or a renderer that cannot start are reported as unavailable/failed,
never as a pass. App logs, screenshots, actual/expected bytes, and reload results
are exported in `guest-results.tar`. A VM failure can prevent that export;
the outer run logs and retained disposable disk remain diagnostic evidence.

Zed runs with software rendering explicitly allowed (`ZED_ALLOW_EMULATED_GPU=1`).
The guarded runner enables `session.trust_all_worktrees` only in its disposable
synthetic-file profile, so startup prompts cannot consume the first fixture line.
It does not change the normal Zed profile. This is a software-renderer input test,
not a hardware GPU validation. See [Zed worktree trust](https://zed.dev/docs/worktree-trust).

The guest enables Omarchy Stay Awake before package setup, preventing idle lock
during downloads. The input guard also checks shell lock state: Hyprland can
retain an application's active-window address behind the lock screen. A locked
session fails before typing rather than being treated as a focused editor.
Stay Awake exists only in the disposable guest and disappears with that overlay.

With `--user-data-dir`, Zed reads settings from `<profile>/config/settings.json`
(the override takes precedence over `XDG_CONFIG_HOME`). The guest exports that
settings file and Zed's internal logs with its other evidence.
