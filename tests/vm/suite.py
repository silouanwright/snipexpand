"""SnipExpand's checks; Oligarchy supplies VM lifecycle and real guest input."""
import json
import os
import time

BUILD = [['cargo', 'build', '--locked', '--release', '--bin', 'snipexpand'],
         ['cargo', 'build', '--locked', '--example', 'e2e_type']]
PAYLOAD = ['target/release/snipexpand', 'target/debug/examples/e2e_type',
           'tests/compatibility', 'contrib/71-snipexpand-keyboard.rules', 'tests/vm/guest-campaign.py', 'tests/vm/guest-packages.py']


def application_result(output):
    # SSH stderr can follow the final JSON line (for example pacman warnings).
    for line in reversed(output.splitlines()):
        try:
            result = json.loads(line)
        except ValueError:
            continue
        if isinstance(result, dict) and 'application' in result and 'passed' in result:
            return result
    raise ValueError('No application result JSON in guest output')


def timing():
    profile = os.environ.get('SNIPEXPAND_VM_TIMING', 'standard')
    if profile not in ('standard', 'stress'):
        raise ValueError('Unknown SNIPEXPAND_VM_TIMING: ' + profile)
    return (90, 500) if profile == 'standard' else (15, 250)


def campaign(h):
    h.logged('packages', 'python3 ~/snipexpand/tests/vm/guest-packages.py', timeout=780)
    h.logged('install', 'python3 ~/guest-setup.py')
    event_delay, pause = timing()
    results = []
    for app in ('firefox', 'zed'):
        try:
            output = h.logged(app, f'python3 ~/snipexpand/tests/vm/guest-campaign.py {app} --event-delay {event_delay} --pause {pause}', timeout=300)
            result = application_result(output.decode())
        except Exception as error:
            try:
                result = application_result((h.out / (app + '.log')).read_text())
            except (OSError, ValueError, IndexError):
                result = {'passed': False, 'error': str(error)}
        (h.out / f'{app}-result.json').write_text(json.dumps(result, indent=2) + '\n')
        # Finish independent application coverage even when another application fails.
        results.append((app, result))
    for app, result in results:
        h.checks.append({'name': app, **result})
    (h.out / 'checks.json').write_text(json.dumps(h.checks, indent=2) + '\n')
    if not all(result['passed'] for _, result in results):
        raise RuntimeError('Firefox/Zed campaign has failed or unavailable cases; see per-app evidence')


def run(h):
    selection = os.environ.get('SNIPEXPAND_VM_CAMPAIGN', 'full')
    if selection not in ('full', 'core', 'firefox-zed'):
        raise ValueError('Unknown SNIPEXPAND_VM_CAMPAIGN: ' + selection)
    if selection == 'firefox-zed':
        return campaign(h)
    h.logged('packages', 'python3 ~/snipexpand/tests/vm/guest-packages.py', timeout=60)
    failures = []
    if selection == 'full':
        try:
            campaign(h)
        except Exception as error:
            h.checks.append({'name': 'firefox-zed-suite', 'passed': False, 'error': str(error)})
            failures.append(str(error))
    # Native QMP setup intentionally keeps its daemon alive until guest teardown.
    # Run it last; Firefox/Zed clean up their own subprocesses before returning.
    try:
        core(h)
    except Exception as error:
        h.checks.append({'name': 'core-suite', 'passed': False, 'error': str(error)})
        failures.append(str(error))
    (h.out / 'checks.json').write_text(json.dumps(h.checks, indent=2) + '\n')
    if failures:
        raise RuntimeError('; '.join(failures))


def core(h):
    event_delay, pause = timing()
    h.logged('install', 'python3 ~/guest-setup.py')
    for app in ('nvim', 'chromium'):
        output=h.logged(app, f'python3 ~/guest-test.py {app} --event-delay {event_delay} --pause {pause}', timeout=240)
        result=application_result(output.decode())
        h.record(app, result['passed'], bytes=result.get('bytes'), guest_artifacts=result['artifacts'],
                 event_delay_ms=result['event_delay_ms'], expansion_pause_ms=result['expansion_pause_ms'])
    h.logged('native-setup', 'python3 ~/native-setup.py')
    h.client('intent','start','--test-result-id',h.agent,'--message','Native ASCII, Unicode, and multiline expansion')
    target=json.loads(h.remote('python3','/home/omarchy/guest-desktop.py','hyprctl','activewindow','-j'))
    if target.get('class') != 'chromium' or 'SnipExpand compatibility target' not in target.get('title',''):
        raise RuntimeError('Guest Chromium fixture is not focused')
    h.screenshot('native-before')
    # The fixture HTML auto-focuses its textarea; no host coordinates or input.
    for keys in (';ascii ', '<ENTER>;unicode ', '<ENTER>;multi '):
        h.client('send-keys','--keys',keys)
        time.sleep(.3)
    h.screenshot('native-expanded')
    h.client('send-keys','--keys','<C-a><C-c>')
    actual=h.ssh('python3 ~/guest-desktop.py wl-paste --no-newline')
    expected='ASCII 123 !?\n¯\\_(ツ)_/¯ 🧐 café 東京\nfirst line\nsecond line'.encode()
    (h.out/'native-actual.txt').write_bytes(actual)
    (h.out/'native-expected.txt').write_bytes(expected)
    h.record('native-qmp-expansion',actual==expected,bytes=len(actual))
    h.client('intent','end')
