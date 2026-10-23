#!/usr/bin/env python3
"""Update code in an existing local Oligarchy installation; preserve its state."""
import datetime as dt
import fcntl
import os
from pathlib import Path
import shutil

source = Path(__file__).resolve().parent / 'harness'
target = Path(os.environ.get('OLIGARCHY_HARNESS_HOME', str(Path.home() / 'Documents/ChatGPT/SnipExpand/oligarchy-pilot')))
if not (target / 'state/pilot.env').is_file() or not (target / 'repo/qemu-server').is_file():
    raise SystemExit('An existing prepared Oligarchy installation is required; see tests/vm/README.md')
with (target / 'state/run.lock').open('a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    stamp = dt.datetime.now(dt.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    backup = target / 'state/code-backups' / stamp
    backup.mkdir(parents=True)
    for name in ('run.py', 'images.py', 'prepare-image.py'):
        if (target / name).exists():
            shutil.copy2(target / name, backup / name)
        shutil.copy2(source / name, target / name)
print('Updated shared harness code; backup: ' + str(backup))
