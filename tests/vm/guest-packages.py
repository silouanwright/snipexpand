#!/usr/bin/env python3
"""Inspect/install target apps only in the disposable, guarded guest."""
import importlib.util
import json
import os
import time
from pathlib import Path
import shutil
import subprocess as sp
spec = importlib.util.spec_from_file_location('guest', Path.home()/'guest-test.py')
guest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)
guest.require_guest()
guest.session_environment()
os.environ.setdefault('OMARCHY_PATH', '/usr/share/omarchy')
# Package downloads can exceed the guest's idle-lock timeout. Keep this
# disposable session awake until Oligarchy destroys it; never touch the host.
sp.run(['omarchy', 'toggle', 'idle', 'stay-awake'], check=True, timeout=10)
deadline = time.monotonic() + 5
while time.monotonic() < deadline:
    idle = json.loads(sp.check_output(['omarchy-shell', 'idle', 'status'], timeout=5))
    if idle.get('stayAwake'):
        break
    time.sleep(.2)
else:
    raise SystemExit('Guest idle inhibition did not become active')
out = Path.home()/'test-results/packages'
out.mkdir(parents=True, exist_ok=True)
(out/'before.txt').write_bytes(sp.check_output(['pacman', '-Q']))
missing = [package for command, package in [('firefox','firefox'), ('zeditor','zed')] if not shutil.which(command)]
if missing:
    # Use Omarchy's system-package update path, which honors its transaction
    # guard and conflict handling. All of this runs only in the disposable guest.
    with (out/'install.log').open('wb') as log:
        result = sp.run(['sudo', '-S', '-p', '', 'env', 'OMARCHY_UPDATE_UNATTENDED=1', 'omarchy', 'update', 'system-pkgs'],
                        input=b'snipexpand-test\n', stdout=log, stderr=sp.STDOUT, timeout=540)
        if result.returncode == 0:
            result = sp.run(['sudo', '-S', '-p', '', 'omarchy', 'pkg', 'add', *missing],
                            input=b'snipexpand-test\n', stdout=log, stderr=sp.STDOUT, timeout=180)
    print('Package installation exit:', result.returncode, flush=True)
(out/'after.txt').write_bytes(sp.check_output(['pacman', '-Q']))
for command in ['firefox','zeditor']:
    print(command, shutil.which(command), flush=True)
    if shutil.which(command):
        result = sp.run([command, '--version'], capture_output=True, timeout=20)
        print(result.stdout.decode(errors='replace'), flush=True)
