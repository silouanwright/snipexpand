#!/usr/bin/env python3
"""Provision a fresh image, reboot it, and seal only after clean OS shutdowns."""
import argparse
import datetime as dt
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess as sp
import sys
import tarfile
import time

from images import ROOT, image_directory, sha256

spec = importlib.util.spec_from_file_location('runner', ROOT / 'run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('name')
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--resume-preparation', action='store_true',
                        help='Retry this named provisioning disk, never a test overlay')
    args = parser.parse_args()
    final = image_directory(args.name)
    if final.exists():
        parser.error('Version already exists; choose a new name')
    stage = final.with_name('.preparing-' + args.name)
    stage.mkdir(parents=True, mode=0o700, exist_ok=args.resume_preparation)
    lock = os.open(ROOT / 'state/run.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    for port in (24223, 42079, 39432):
        with socket.socket() as probe:
            if probe.connect_ex(('127.0.0.1', port)) == 0:
                raise RuntimeError(f'Port {port} is occupied; refusing to reuse it')
    base = ROOT / 'data/snipexpand.iso.qcow2'
    firmware = ROOT / 'data/snipexpand.iso.OVMF_VARS.fd'
    before = {str(p): sha256(p) for p in (base, firmware)}
    disk, vars_file = stage / 'provision.qcow2', stage / 'desktop.iso.OVMF_VARS.fd'
    if args.resume_preparation:
        info = json.loads(sp.check_output(['qemu-img', 'info', '--output=json', disk]))
        if (info.get('backing-filename') != str(base) or not vars_file.is_file()
                or not (stage / 'setup.tar').is_file()):
            raise ValueError('Not an intact provisioning directory from this original base')
        runner.command(['qemu-img', 'check', disk])
    else:
        runner.command(['qemu-img', 'create', '-f', 'qcow2', '-F', 'qcow2', '-b', base, disk])
        shutil.copyfile(firmware, vars_file)
    guest = runner.Suite('image-preparation', args.source.resolve(), stage)
    process = None
    def terminate(signum, frame):
        raise KeyboardInterrupt(f'Received signal {signum}')
    signal.signal(signal.SIGTERM, terminate)

    def boot(number):
        nonlocal process
        argv = ['qemu-system-x86_64', '-machine', 'q35,accel=kvm', '-cpu', 'host',
                '-smp', '2', '-m', '4G', '-smbios', 'type=1,serial=snipexpand-test-vm',
                '-drive', 'if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd',
                '-drive', f'if=pflash,format=raw,file={vars_file}',
                '-drive', f'file={disk},if=virtio,format=qcow2',
                '-display', 'none', '-vga', 'none', '-device', 'virtio-vga',
                '-device', 'qemu-xhci', '-device', 'usb-tablet',
                '-netdev', 'user,id=pilot,hostfwd=tcp:127.0.0.1:24223-:22',
                '-device', 'virtio-net-pci,netdev=pilot',
                '-serial', f'file:{stage}/serial-{number}.log']
        runner.write_json(stage / f'qemu-{number}.json', argv)
        with (stage / f'qemu-{number}.log').open('wb') as log:
            process = sp.Popen(argv, stdout=log, stderr=sp.STDOUT)
        guest.ready()

    def poweroff():
        # Guarded SSH is the only shutdown path. A killed or hung QEMU cannot be sealed.
        guest.logged('poweroff', 'python3 /home/omarchy/image-poweroff.py')
        code = process.wait(timeout=90)
        if code != 0:
            raise RuntimeError(f'QEMU did not shut down cleanly: {code}')

    try:
        boot(1)
        provisioner = args.source.resolve() / 'tests/vm/guest-packages.py'
        provisioner_digest = sha256(provisioner)
        shutdown = stage / 'image-poweroff.py'
        shutdown.write_text("import guest_test_guard as guest\nimport subprocess as sp\n"
                            "guest.require_guest()\n"
                            "sp.run(['sudo','-S','-p','','systemd-run','--on-active=3',"
                            "'--unit=snipexpand-image-poweroff','systemctl','poweroff'], "
                            "input=b'snipexpand-test\\n', check=True)\n")
        # This archive contains only guarded guest helpers, never host state or credentials.
        with tarfile.open(stage / 'setup.tar', 'w') as tar:
            tar.add(ROOT.parent / 'vm/guest-test.py', arcname='guest_test_guard.py')
            tar.add(ROOT.parent / 'vm/guest-test.py', arcname='guest-test.py')
            tar.add(provisioner, arcname='guest-packages.py')
            tar.add(shutdown, arcname='image-poweroff.py')
        with (stage / 'setup.tar').open('rb') as stream:
            runner.command([*guest.ssh_args, 'tar -xf - -C "$HOME"'], stdin=stream, capture_output=True)
        print('Provisioning apps in a fresh guest; evidence: ' + str(stage), flush=True)
        guest.logged('packages-install', 'python3 ~/guest-packages.py --install', timeout=1200)
        poweroff()
        print('Rebooting the provisioned disk to verify the installed kernel and apps', flush=True)
        boot(2)
        guest.logged('packages-verify', 'python3 ~/guest-packages.py', timeout=60)
        packages = guest.remote('pacman', '-Q')
        kernel = guest.remote('uname', '-r').strip()
        # Save the package database and application versions for repeatable image selection.
        (stage / 'packages.txt').write_text(packages)
        (stage / 'guest-results.tar').write_bytes(guest.ssh('tar -cf - -C "$HOME" test-results/packages'))
        poweroff()
        runner.command(['qemu-img', 'check', disk])
        runner.command(['qemu-img', 'convert', '-O', 'qcow2', disk, stage / 'desktop.iso.qcow2'], timeout=600)
        runner.command(['qemu-img', 'check', stage / 'desktop.iso.qcow2'])
        if any(sha256(Path(p)) != digest for p, digest in before.items()):
            raise RuntimeError('Original image changed during provisioning')
        manifest = {'schema': 1, 'name': args.name, 'sealed': True,
                    'created_at': dt.datetime.now(dt.timezone.utc).isoformat(),
                    'base_sha256': before, 'kernel': kernel, 'clean_shutdowns': 2,
                    'provisioner_sha256': provisioner_digest,
                    'packages': packages.splitlines(),
                    'sha256': {p: sha256(stage / p) for p in ('desktop.iso.qcow2', 'desktop.iso.OVMF_VARS.fd')}}
        runner.write_json(stage / 'manifest.json', manifest)
        # The original provisioning overlay is retained for diagnosis; only the
        # independent converted disk is selected by resolve_image().
        stage.rename(final)
        print('Sealed image (not yet active): ' + str(final), flush=True)
    except BaseException as error:
        runner.write_json(stage / 'failure.json', {'error': str(error), 'sealed': False})
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=15)
            except sp.TimeoutExpired:
                process.kill()
                process.wait()
        os.close(lock)


if __name__ == '__main__':
    main()
