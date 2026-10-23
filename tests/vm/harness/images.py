"""Versioned, standalone VM images. No test overlay is ever promoted here."""
import hashlib
import argparse
import fcntl
import json
from pathlib import Path
import re
import os
import subprocess as sp

ROOT = Path(__file__).resolve().parent


def sha256(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def image_directory(name):
    if not re.fullmatch(r'[a-z0-9][a-z0-9.-]{0,63}', name):
        raise ValueError('Image name must be 1–64 lowercase letters, digits, dots or hyphens')
    return ROOT / 'data/images' / name


def resolve_image(name=None):
    pointer = ROOT / 'data/active-image.json'
    expected_manifest = None
    if name is None:
        if not pointer.exists():
            return ROOT / 'data/snipexpand.iso', None
        selected = json.loads(pointer.read_text())
        name = selected['name']
        expected_manifest = selected['manifest_sha256']
    directory = image_directory(name)
    manifest_path = directory / 'manifest.json'
    if expected_manifest is not None and sha256(manifest_path) != expected_manifest:
        raise ValueError('Active image manifest changed after activation')
    manifest = json.loads(manifest_path.read_text())
    if manifest.get('schema') != 1 or manifest.get('name') != name or manifest.get('sealed') is not True:
        raise ValueError('Image manifest is not a sealed v1 image')
    for filename in ('desktop.iso.qcow2', 'desktop.iso.OVMF_VARS.fd'):
        if sha256(directory / filename) != manifest['sha256'][filename]:
            raise ValueError(f'Image checksum mismatch: {filename}')
    info = json.loads(sp.check_output(['qemu-img', 'info', '--output=json', directory / 'desktop.iso.qcow2']))
    if info.get('backing-filename') or info.get('format') != 'qcow2':
        raise ValueError('Prepared image must be a standalone qcow2')
    return directory / 'desktop.iso', {**manifest, 'manifest_sha256': sha256(manifest_path)}


def activate(name, results):
    _, manifest = resolve_image(name)
    projects = {r.get('project'): r for r in results}
    if set(projects) != {'snipexpand', 'look-elsewhere'} or len(results) != 2:
        raise ValueError('Activation requires a fresh passing run of both shared projects')
    required = {'nvim', 'chromium', 'native-qmp-expansion', 'firefox', 'zed'}
    for project, result in projects.items():
        if result.get('passed') is not True or result.get('cleanup_errors'):
            raise ValueError('Image validation failed: ' + project)
        if result.get('image', {}).get('manifest_sha256') != manifest['manifest_sha256']:
            raise ValueError('Validation used a different image')
        checks = result.get('checks', [])
        if not checks or any(c.get('passed') is not True for c in checks):
            raise ValueError('Missing or failed image validation checks')
        if project == 'snipexpand' and {c['name'] for c in checks} != required:
            raise ValueError('Activation requires the complete SnipExpand suite')
    pointer = ROOT / 'data/active-image.json'
    previous = json.loads(pointer.read_text()) if pointer.exists() else None
    staged = pointer.with_suffix('.json.tmp')
    staged.write_text(json.dumps({'name': name, 'manifest_sha256': manifest['manifest_sha256'],
                                 'previous': previous, 'validation': results}, indent=2) + '\n')
    staged.replace(pointer)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Activate a verified versioned image')
    parser.add_argument('name')
    parser.add_argument('--results', type=Path, required=True)
    args = parser.parse_args()
    with (ROOT / 'state/run.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        activate(args.name, json.loads(args.results.read_text()))
    print('Active image: ' + args.name)
