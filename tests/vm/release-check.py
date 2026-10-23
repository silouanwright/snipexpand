#!/usr/bin/env python3
"""Run the full local VM suite and attest its exact commit for release gating."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess as sp
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
CONTEXT = 'desktop-vm/full'
PREFIX = 'VM full v1 '
REQUIRED = {'nvim', 'chromium', 'native-qmp-expansion', 'firefox', 'zed'}


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def git(*args):
    return sp.check_output(['git', '-C', str(ROOT), *args], text=True).strip()


def clean_commit():
    if git('status', '--porcelain', '--untracked-files=all'):
        raise ValueError('Release checks require a clean checkout, including untracked files')
    return git('rev-parse', 'HEAD')


def validate_result(results, commit):
    if len(results) != 1:
        raise ValueError('Expected exactly one SnipExpand result')
    result = results[0]
    if result.get('project') != 'snipexpand' or result.get('passed') is not True or result.get('cleanup_errors'):
        raise ValueError('VM run failed or cleanup was incomplete')
    checks = result.get('checks', [])
    names = [check.get('name') for check in checks]
    if set(names) != REQUIRED or len(names) != len(REQUIRED) or any(c.get('passed') is not True for c in checks):
        raise ValueError('All five graphical checks must pass exactly once')
    for check in checks:
        if check['name'] != 'native-qmp-expansion' and (check.get('event_delay_ms'), check.get('expansion_pause_ms')) != (90, 500):
            raise ValueError('Release checks require the standard 90/500 ms input timing')
    image = result.get('image') or {}
    if image.get('sealed') is not True or not image.get('manifest_sha256'):
        raise ValueError('Release checks require a sealed, versioned image')
    evidence = Path(result['artifacts'])
    manifest = json.loads((evidence / 'payload-manifest.json').read_text())
    if manifest.get('commit') != commit or manifest.get('status', '').strip():
        raise ValueError('Payload is dirty or belongs to a different commit')
    if Path(manifest['source']).resolve() != ROOT:
        raise ValueError('Payload belongs to another checkout')
    required_payload = {'target/release/snipexpand', 'target/debug/examples/e2e_type',
                        'tests/vm/guest-campaign.py', 'tests/vm/guest-packages.py'}
    if not required_payload.issubset(manifest.get('sha256', {})) or not manifest.get('harness_sha256'):
        raise ValueError('Incomplete source or harness provenance')
    for relative, expected in manifest['sha256'].items():
        path = (ROOT / relative).resolve()
        if not path.is_relative_to(ROOT) or digest(path) != expected:
            raise ValueError('Payload changed since testing: ' + relative)
    for filename, expected in manifest['harness_sha256'].items():
        if digest(filename) != expected:
            raise ValueError('Harness changed since testing: ' + filename)
    return evidence, image


def check_status(data, commit, actor):
    if data.get('sha') != commit:
        raise ValueError('Status response is for another commit')
    matches = [s for s in data.get('statuses', []) if s.get('context') == CONTEXT]
    if not matches:
        raise ValueError('Missing desktop-vm/full status; run and publish tests/vm/release-check.py first')
    latest = max(matches, key=lambda s: s['id'])
    if (latest.get('state') != 'success' or
            latest.get('creator', {}).get('login') != actor or
            not re.fullmatch(re.escape(PREFIX) + r'[a-f0-9]{64}', latest.get('description', ''))):
        raise ValueError('Latest graphical status is not a successful trusted v1 attestation')


def run(args):
    commit = clean_commit()
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        raise ValueError('Put release evidence outside the checkout to keep the source clean')
    output.mkdir(parents=True, mode=0o700)
    summary = output / 'summary.json'
    receipt = output / 'receipt.json'
    if summary.exists() or receipt.exists():
        raise ValueError('Evidence already exists; choose a fresh output directory')
    env = {**os.environ, 'SNIPEXPAND_VM_CAMPAIGN': 'full', 'SNIPEXPAND_VM_TIMING': 'standard',
           'SNIPEXPAND_VM_CAMPAIGN_REPEATS': '1'}
    command = [sys.executable, str(ROOT / 'tests/vm/run.py'), '--result-file', str(summary)]
    if args.image:
        command.extend(['--image', args.image])
    sp.run(command, env=env, check=True)
    if clean_commit() != commit:
        raise ValueError('Source changed during testing')
    evidence, image = validate_result(json.loads(summary.read_text()), commit)
    files = [summary, *sorted(p for p in evidence.rglob('*') if p.is_file())]
    if not any(p.name == 'guest-results.tar' for p in files):
        raise ValueError('Missing exported guest evidence')
    data = {'schema': 1, 'commit': commit, 'source': str(ROOT), 'context': CONTEXT,
            'completed_at': dt.datetime.now(dt.timezone.utc).isoformat(),
            'image': image['name'], 'image_manifest_sha256': image['manifest_sha256'],
            'summary': str(summary), 'files': {str(p): digest(p) for p in files}}
    receipt.write_text(json.dumps(data, indent=2) + '\n')
    print('Full graphical release check passed. Local receipt: ' + str(receipt))


def publish(args):
    receipt = args.receipt.resolve()
    data = json.loads(receipt.read_text())
    commit = clean_commit()
    if data.get('schema') != 1 or data.get('commit') != commit or data.get('source') != str(ROOT):
        raise ValueError('Receipt is not for this clean checkout and commit')
    for filename, expected in data['files'].items():
        if digest(filename) != expected:
            raise ValueError('Evidence changed: ' + filename)
    summary = Path(data['summary'])
    if str(summary) not in data['files']:
        raise ValueError('Receipt does not hash its summary')
    validate_result(json.loads(summary.read_text()), commit)
    origin = git('remote', 'get-url', 'origin')
    match = re.fullmatch(r'(?:https://github.com/|git@github.com:)([\w.-]+/[\w.-]+?)(?:\.git)?', origin)
    if not match:
        raise ValueError('Expected a GitHub origin remote')
    repository = match.group(1)
    actor = sp.check_output(['gh', 'api', 'user', '--jq', '.login'], text=True).strip()
    expected_actor = os.environ.get('VM_GATE_ACTOR') or repository.split('/')[0]
    if actor != expected_actor:
        raise ValueError(f'Switch gh to {expected_actor} before publishing the release status')
    # Publishing is explicit: ordinary test commands never contact GitHub.
    sp.run(['gh', 'api', '--method', 'POST', f'repos/{repository}/statuses/{commit}',
            '-f', 'state=success', '-f', 'context=' + CONTEXT,
            '-f', 'description=' + PREFIX + digest(receipt)], check=True)


def verify_status(args):
    repository, commit = os.environ['GITHUB_REPOSITORY'], os.environ['GITHUB_SHA']
    actor = os.environ.get('VM_GATE_ACTOR') or repository.split('/')[0]
    url = f'https://api.github.com/repos/{repository}/commits/{commit}/status?per_page=100'
    request = urllib.request.Request(url, headers={
        'Authorization': 'Bearer ' + os.environ['GH_TOKEN'],
        'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28'})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.load(response)
    check_status(data, commit, actor)
    print(f'Graphical release gate passed for {commit}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    run_parser = sub.add_parser('run', help='Run the full suite and save a local receipt')
    run_parser.add_argument('--output', type=Path, required=True)
    run_parser.add_argument('--image')
    pub = sub.add_parser('publish', help='Explicitly publish a passing receipt as a GitHub commit status')
    pub.add_argument('receipt', type=Path)
    sub.add_parser('verify-status', help='Release workflow: require the exact commit status')
    args = parser.parse_args()
    {'run': run, 'publish': publish, 'verify-status': verify_status}[args.command](args)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, sp.SubprocessError) as error:
        raise SystemExit(str(error))
