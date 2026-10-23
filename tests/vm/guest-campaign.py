#!/usr/bin/env python3
"""Run one real-input fixture, refusing execution outside the provisioned VM."""
import argparse
import datetime
import difflib
import json
import os
from pathlib import Path
import signal
import shutil
import stat
import subprocess as sp
import tempfile
import time


def require_guest():
    marker = Path('/etc/snipexpand-test-vm')
    try:
        metadata = marker.stat()
        valid = (metadata.st_uid == 0 and not metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
                 and marker.read_text().strip() == 'snipexpand-test-vm')
    except FileNotFoundError:
        valid = False
    virt = sp.run(['systemd-detect-virt', '--vm'], capture_output=True, text=True)
    if not valid or virt.returncode or virt.stdout.strip() not in ('kvm', 'qemu'):
        raise SystemExit('REFUSED: live input requires the provisioned SnipExpand QEMU/KVM guest.')
    if os.getuid() == 0:
        raise SystemExit('REFUSED: run as the guest graphical user, not root.')


def session_environment():
    os.environ['XDG_RUNTIME_DIR'] = f'/run/user/{os.getuid()}'
    os.environ['DBUS_SESSION_BUS_ADDRESS'] = f'unix:path=/run/user/{os.getuid()}/bus'
    instances = json.loads(sp.check_output(['hyprctl', 'instances', '-j']))
    if len(instances) != 1:
        raise RuntimeError(f'Expected one guest Hyprland session; found {len(instances)}')
    instance = instances[0]
    os.environ.update(HYPRLAND_INSTANCE_SIGNATURE=instance['instance'],
                      WAYLAND_DISPLAY=instance['wl_socket'], XDG_SESSION_TYPE='wayland')
    exported = sp.check_output(['systemctl', '--user', 'show-environment'], text=True)
    for line in exported.splitlines():
        key, sep, value = line.partition('=')
        if sep and key in ('DISPLAY', 'XMODIFIERS', 'GTK_IM_MODULE', 'QT_IM_MODULE', 'OMARCHY_PATH'):
            os.environ[key] = value
    os.environ.setdefault('OMARCHY_PATH', '/usr/share/omarchy')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('application', choices=['firefox', 'zed'])
    parser.add_argument('--repo', type=Path, default=Path.home() / 'snipexpand')
    parser.add_argument('--event-delay', type=int, default=15)
    parser.add_argument('--pause', type=int, default=250)
    parser.add_argument('--fixture', choices=['compatibility', 'undo'], default='compatibility')
    args = parser.parse_args()
    require_guest()  # Must precede all desktop, service, clipboard, and input operations.
    session_environment()
    repo = args.repo.resolve()
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    out = Path.home() / 'test-results' / f'{stamp}-{args.application}'
    out.mkdir(parents=True)
    (out / 'packages.txt').write_bytes(sp.check_output(['pacman', '-Q']))
    (out / 'session.json').write_text(json.dumps({key: os.environ.get(key) for key in
        ('WAYLAND_DISPLAY', 'HYPRLAND_INSTANCE_SIGNATURE', 'GTK_IM_MODULE', 'QT_IM_MODULE')}, indent=2))

    def state(which):
        if which == 'activewindow':
            # A session lock can leave Hyprland's last application address intact.
            lock = json.loads(sp.check_output(['omarchy-shell', 'lock', 'status'], timeout=5))
            if any(lock.get(key) for key in ('locked', 'requested', 'pending', 'sessionLocked', 'secure')):
                raise RuntimeError('Guest session is locked; input refused')
        return json.loads(sp.check_output(['hyprctl', which, '-j']))

    def dispatch(action, address):
        sp.run(['hyprctl', 'dispatch', 'hl.dsp.' + action + '({ window = ' +
                json.dumps('address:' + address) + ' })'], check=True, stdout=sp.DEVNULL)

    def capture(name):
        sp.run(['grim', str(out / (name + '.png'))], stdout=sp.DEVNULL, stderr=sp.DEVNULL)

    config = out / 'config'
    shutil.copytree(repo / 'tests/compatibility/config', config)
    cache = Path(tempfile.mkdtemp(prefix='sx-vm-'))
    (out / 'profile-cache.txt').write_text(str(cache) + '\n')
    daemon = app = driver = None
    target = None
    result = {'application': args.application, 'passed': False,
              'event_delay_ms': args.event_delay, 'expansion_pause_ms': args.pause,
              'fixture': args.fixture}
    try:
        # Refuse a second daemon instead of manipulating an unrelated user service.
        if sp.run(['pgrep', '-x', 'snipexpand'], stdout=sp.DEVNULL).returncode == 0:
            raise RuntimeError('A SnipExpand daemon is already running in the guest')
        known = {w['address'] for w in state('clients')}
        env = os.environ.copy()
        env.update(XDG_CONFIG_HOME=str(config), RUST_LOG='snipexpand::daemon=debug,snipexpand::keyboard=debug',
                   SNIPEXPAND_E2E_EVENT_DELAY_MS=str(args.event_delay),
                   SNIPEXPAND_E2E_EXPANSION_PAUSE_MS=str(args.pause))
        with (out / 'daemon.log').open('wb') as dlog, (out / 'application.log').open('wb') as alog:
            daemon = sp.Popen([str(repo / 'target/release/snipexpand')], env=env, stdout=dlog, stderr=sp.STDOUT, start_new_session=True)
            time.sleep(1)
            if daemon.poll() is not None:
                raise RuntimeError('Guest daemon exited during startup')
            app_env = os.environ.copy()
            # Every launch gets new editor state and browser storage, including repeats.
            app_env['SNIPEXPAND_COMPATIBILITY_ROOT'] = str(cache)
            if args.application == 'zed':
                # Synthetic file, disposable profile; leave the host Zed settings alone.
                app_env.update(SNIPEXPAND_VM_FIXTURE='1', ZED_ALLOW_EMULATED_GPU='1')
            app = sp.Popen([str(repo / 'tests/compatibility/launch-target.sh'), args.application], env=app_env, stdout=alog, stderr=sp.STDOUT, start_new_session=True)
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                targets = [w for w in state('clients') if w['address'] not in known and
                           ('SnipExpand compatibility target' in w['title'] or (args.application == 'zed' and 'zed-target.txt' in w['title']))]
                if targets:
                    target = targets[0]
                    break
                if app.poll() is not None and app.returncode != 0:
                    raise RuntimeError('Application launcher failed; see application.log')
                time.sleep(.2)
            if target is None:
                raise RuntimeError('No target window appeared')
            dispatch('focus', target['address'])
            time.sleep(1)
            for _ in range(30):
                if state('activewindow').get('address') != target['address']:
                    raise RuntimeError('Guest focus changed before input')
                time.sleep(.1)
            capture('before')
            sp.run(['wl-copy', '--clear'], check=True, timeout=5)
            driver = sp.Popen([str(repo / 'target/debug/examples/e2e_type'),
                               '--undo-stress-copy' if args.fixture == 'undo' else '--compatibility-copy'],
                              env=env, stdout=alog, stderr=sp.STDOUT, start_new_session=True)
            deadline = time.monotonic() + 180
            while driver.poll() is None:
                if time.monotonic() > deadline:
                    raise RuntimeError('Input driver timed out')
                if state('activewindow').get('address') != target['address']:
                    raise RuntimeError('Guest focus changed during input; result discarded')
                time.sleep(.05)
            if driver.wait(timeout=5) != 0:
                raise RuntimeError('Input driver failed')
            expected = (repo / 'tests/compatibility/expected.txt').read_bytes()
            if state('activewindow').get('address') != target['address']:
                raise RuntimeError('Guest focus changed before capture')
            actual = sp.check_output(['wl-paste', '--no-newline'], timeout=5)
            expected = (b'SENTINEL\n' + b';undo \n' * 20) if args.fixture == 'undo' else expected[:-1]
            (out / 'actual.txt').write_bytes(actual)
            (out / 'expected.txt').write_bytes(expected)
            result.update(passed=actual == expected, bytes=len(actual),
                          target={k: target[k] for k in ('class', 'title')})
            if actual != expected:
                (out / 'diff.txt').write_text(''.join(difflib.unified_diff(
                    expected.decode(errors='replace').splitlines(True), actual.decode(errors='replace').splitlines(True))))
                result['fixture_error'] = 'Output differs from expected.txt'
            capture('fixture')
            result['fixture_passed'] = actual == expected
            if args.application == 'firefox':
                # Export DOM input ordering after the ordinary byte comparison.
                sp.run([str(repo / 'target/debug/examples/e2e_type'), '--copy-event-log'],
                       env=env, check=True, timeout=15, stdout=alog, stderr=sp.STDOUT)
                trace = sp.check_output(['wl-paste', '--no-newline'], timeout=5)
                json.loads(trace)  # Refuse an empty or stale clipboard capture.
                (out / 'input-events.json').write_bytes(trace)
            result['reload'] = []
            reload_file = config / 'snipexpand/match/reload.yml'
            original_pid = daemon.pid
            for stage, replacement in [('add', 'reload 🦊 東京'), ('change', 'changed 🚀 café'), ('remove', None)]:
                reload_file.write_text('matches: []\n' if replacement is None else
                    'matches:\n  - trigger: ";reload"\n    replace: ' + json.dumps(replacement, ensure_ascii=False) + '\n')
                reload_reply = sp.check_output([str(repo / 'target/release/snipexpand'), 'reload'], env=env, text=True)
                time.sleep(.7)
                if daemon.poll() is not None or daemon.pid != original_pid:
                    raise RuntimeError('Daemon exited during reload')
                if state('activewindow').get('address') != target['address']:
                    raise RuntimeError('Focus changed before reload input')
                sp.run(['wl-copy', '--clear'], check=True, timeout=5)
                driver = sp.Popen([str(repo / 'target/debug/examples/e2e_type'), '--probe-copy', ';reload'],
                                  env=env, stdout=alog, stderr=sp.STDOUT, start_new_session=True)
                probe_deadline = time.monotonic() + 20
                while driver.poll() is None:
                    if time.monotonic() > probe_deadline:
                        raise RuntimeError('Reload probe timeout')
                    if state('activewindow').get('address') != target['address']:
                        raise RuntimeError('Focus changed during reload probe')
                    time.sleep(.05)
                if driver.returncode:
                    raise RuntimeError('Reload probe failed')
                if state('activewindow').get('address') != target['address']:
                    raise RuntimeError('Focus changed before reload capture')
                observed = sp.check_output(['wl-paste', '--no-newline'], timeout=5)
                wanted = (replacement if replacement is not None else ';reload ').encode()
                (out / (stage + '-actual.txt')).write_bytes(observed)
                (out / (stage + '-expected.txt')).write_bytes(wanted)
                result['reload'].append({'stage': stage, 'passed': observed == wanted, 'reply': reload_reply.strip()})
                capture(stage)
            result['passed'] = result['fixture_passed'] and all(c['passed'] for c in result['reload'])
            result['daemon_pid_unchanged'] = daemon.pid == original_pid and daemon.poll() is None
    except Exception as error:
        result.update(passed=False, error=str(error))
        try:
            capture('failure')
        except Exception as capture_error:
            result['screenshot_error'] = str(capture_error)
    finally:
        # These processes and their children were created by this runner only.
        for process in (driver, app, daemon):
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except sp.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
        if args.application == 'zed':
            profile = cache / 'zed-profile'
            if (profile / 'config/settings.json').is_file():
                shutil.copy2(profile / 'config/settings.json', out / 'zed-settings.json')
            if (profile / 'logs').is_dir():
                shutil.copytree(profile / 'logs', out / 'zed-logs', dirs_exist_ok=True)
        (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps({'artifacts': str(out), **result}), flush=True)
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
