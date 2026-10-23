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


def campaign(h):
    h.logged('packages', 'python3 ~/snipexpand/tests/vm/guest-packages.py', timeout=780)
    h.logged('install', 'python3 ~/guest-setup.py')
    results = []
    for app in ('firefox', 'zed'):
        try:
            output = h.logged(app, f'python3 ~/snipexpand/tests/vm/guest-campaign.py {app}', timeout=300)
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
    if os.environ.get('SNIPEXPAND_VM_CAMPAIGN') == 'firefox-zed':
        return campaign(h)
    h.logged('install', 'python3 ~/guest-setup.py')
    for app in ('nvim', 'chromium'):
        output=h.logged(app, f'python3 ~/guest-test.py {app}', timeout=240)
        result=json.loads(output.splitlines()[-1])
        h.record(app, result['passed'], bytes=result.get('bytes'), guest_artifacts=result['artifacts'])
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
