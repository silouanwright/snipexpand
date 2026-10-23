"""SnipExpand's checks; Oligarchy supplies VM lifecycle and real guest input."""
import json
import time

BUILD = [['cargo', 'build', '--locked', '--release', '--bin', 'snipexpand'],
         ['cargo', 'build', '--locked', '--example', 'e2e_type']]
PAYLOAD = ['target/release/snipexpand', 'target/debug/examples/e2e_type',
           'tests/compatibility', 'contrib/71-snipexpand-keyboard.rules']


def run(h):
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
