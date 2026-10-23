# Graphical tests and the release gate

```bash
python3 tests/vm/run.py
```

The default suite runs Firefox, Zed, Neovim, Chromium, and native Oligarchy
keyboard input in a disposable Omarchy guest. Fixtures compare exact UTF-8 bytes
for ASCII, Unicode/emoji, multiline text, cursor placement, undo, and repeated
expansion. Firefox and Zed also test add/change/remove reloads while the original
daemon stays alive. Failed checks and incomplete cleanup return nonzero.

All keyboard, clipboard, focus, service, and installation operations belong to
the guarded guest. There is no host desktop fallback. A root-owned marker,
QEMU/KVM detection, pinned SSH host keys, and application focus guards protect
the input path. These tests do not cover physical keyboards, GPU hardware,
suspend/hotplug, non-US layouts, or the full IME matrix.

## Existing local installation

The wrapper uses `~/Documents/ChatGPT/SnipExpand/oligarchy-pilot`, or
`OLIGARCHY_HARNESS_HOME`. A new machine still needs the prepared Omarchy base,
Oligarchy dependencies, local PostgreSQL cluster, guest SSH key, and KVM. See
the installation's `AUTOMATION.md`. Credentials and VM disks stay outside this
repository. This command does not register a GitHub self-hosted runner or a timer.

Update an existing installation's shared runner code from this checkout:

```bash
python3 tests/vm/install-harness.py
```

The installer backs up the three code files, takes the shared VM lock, and
preserves credentials, disks, database, and guest helper files. The tracked
copies are under `tests/vm/harness/`. The installation's guest helpers are also
hashed in each payload manifest, so changing them invalidates a release receipt.

On Silouan's machine, the shared runner automatically uses the Synaxis build
budget wrapper (two CPUs, idle I/O, serialized heavy work). Set
`SNIPEXPAND_BUILD_BUDGET` to use another wrapper. The VM has its own exclusive
lock and refuses occupied ports instead of touching unrelated services.

## Prepared images

Normal tests **never install or update guest packages**. Prepare a new version
explicitly when applications need updating:

```bash
HARNESS="$HOME/Documents/ChatGPT/SnipExpand/oligarchy-pilot"
/home/silouan/Work/projects/synaxis/scripts/synaxis-build-budget.sh -- \
  python3 "$HARNESS/prepare-image.py" desktop-YYYYMMDD-v1 --source "$PWD"
python3 "$HARNESS/run.py" all --image desktop-YYYYMMDD-v1 \
  --result-file /tmp/desktop-image-validation.json
python3 "$HARNESS/images.py" desktop-YYYYMMDD-v1 \
  --results /tmp/desktop-image-validation.json
```

Use a new name each time. Preparation starts from the original sealed base in a
fresh overlay, runs Omarchy's system-package update and package installer only
inside that guest, powers off, boots again, verifies applications, then powers
off cleanly a second time. It converts the disk to a standalone qcow2 and records
package versions, booted kernel, original hashes, and final disk/firmware hashes.
Interrupted provisioning directories remain diagnostic evidence and cannot be
selected as sealed images. No prior test overlay is promoted.

Activation requires all SnipExpand checks and LookElsewhere to pass on the same
image, because the installation is shared. The active pointer changes atomically
and retains its previous value. Old image versions remain available. Every run
verifies hashes before booting; `--image NAME` selects a particular version.
Image preparation updates system packages, not the complete Omarchy migration
sequence. Record a new image version after deliberate migration work.

## Release procedure

Commit the release version and all intended changes first. From that clean
checkout, produce a receipt outside the repository:

```bash
python3 tests/vm/release-check.py run \
  --output "$HOME/Documents/ChatGPT/SnipExpand/verification/release-vX.Y.Z"
```

This forces the full suite, rebuilds with `--locked`, and requires all five
checks, a sealed image, clean source, matching payload hashes, and successful
cleanup. It saves a local `receipt.json` plus the usual artifacts. The receipt
hashes all evidence files, including screenshots, logs, the guest archive, and
payload provenance. A changed commit, binary, harness, or evidence file requires
a fresh run. Do not amend the commit after testing.

After pushing that exact commit, explicitly publish its passing status:

```bash
gh auth switch --user silouanwright
python3 tests/vm/release-check.py publish /absolute/path/to/receipt.json
```

Then tag that tested commit and push the tag through the normal release process.
The release workflow checks `desktop-vm/full` on the **exact tag commit** before
either architecture build starts. Missing, pending, failed, or untrusted statuses
block release. The status description contains the receipt's SHA-256; evidence
remains local and should be retained with the release record.

By default, only a status created by the repository owner is accepted. A project
using a dedicated test account can set the repository variable `VM_GATE_ACTOR`
and use the same environment variable when publishing. The account needs commit
status write access. This is an owner-attested local test gate, not a hosted VM
job or cryptographic proof against an owner who deliberately bypasses it. It uses
[GitHub commit statuses](https://docs.github.com/en/rest/commits/statuses).
Normal test runs never write to GitHub, push code, create tags, or publish releases.

GitHub CI runs the gate's negative tests alongside Rust and Fcitx bridge checks.
The tag workflow enforces graphical acceptance once these changes are pushed.

## Focused debugging and evidence

```bash
SNIPEXPAND_VM_CAMPAIGN=core python3 tests/vm/run.py
SNIPEXPAND_VM_CAMPAIGN=firefox-zed python3 tests/vm/run.py
python3 -m unittest discover -s tests/vm -p 'test_*.py'
```

Focused runs do not qualify for a release receipt. Full runs continue to the
other application group if one group fails. Native QMP input runs last because
its fixture daemon remains alive until the guest is disposed.

Evidence lives under the harness's `artifacts/<timestamp>/snipexpand/`: results,
source commit/dirty status, payload and harness hashes, image manifest, screenshots,
guest logs, and Oligarchy action history. A VM crash may prevent guest export;
outer logs and retained disposable disks still provide diagnostic evidence.

Zed uses software rendering (`ZED_ALLOW_EMULATED_GPU=1`) and trusts only the
synthetic files in its disposable profile. Its `--user-data-dir` settings are at
`<profile>/config/settings.json`; the runner exports settings and internal logs.
Firefox has its own clean profile. Guest Stay Awake prevents idle locking, and
Firefox/Zed input guards explicitly reject a locked session even when Hyprland
still reports the previous active window.
