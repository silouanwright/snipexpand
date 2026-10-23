"""Image selection must fail closed on tampering or incomplete validation."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('images', Path(__file__).parent / 'harness/images.py')
images = importlib.util.module_from_spec(spec)
spec.loader.exec_module(images)


class ImageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        p = patch.object(images, 'ROOT', self.root)
        p.start()
        self.addCleanup(p.stop)
        self.directory = images.image_directory('test-v1')
        self.directory.mkdir(parents=True)
        self.manifest = {'schema': 1, 'name': 'test-v1', 'sealed': True, 'sha256': {}}
        for name in ['desktop.iso.qcow2', 'desktop.iso.OVMF_VARS.fd']:
            path = self.directory / name
            path.write_bytes(name.encode())
            self.manifest['sha256'][name] = images.sha256(path)
        (self.directory / 'manifest.json').write_text(json.dumps(self.manifest))
        p = patch.object(images.sp, 'check_output', return_value=b'{"format":"qcow2"}')
        self.qemu = p.start()
        self.addCleanup(p.stop)

    def results(self):
        _, manifest = images.resolve_image('test-v1')
        return [{'project': project, 'passed': True, 'image': manifest,
                 'checks': [{'name': name, 'passed': True} for name in checks]}
                for project, checks in [('snipexpand', ['nvim', 'chromium', 'native-qmp-expansion', 'firefox', 'zed']),
                                        ('look-elsewhere', ['plugin-fixture'])]]

    def test_valid_image_activation(self):
        images.activate('test-v1', self.results())
        _, manifest = images.resolve_image()
        self.assertEqual(manifest['name'], 'test-v1')

    def test_changed_disk_rejected(self):
        (self.directory / 'desktop.iso.qcow2').write_bytes(b'changed')
        with self.assertRaises(ValueError):
            images.resolve_image('test-v1')

    def test_overlay_is_not_a_sealed_base(self):
        self.qemu.return_value = b'{"format":"qcow2","backing-filename":"old-base"}'
        with self.assertRaises(ValueError):
            images.resolve_image('test-v1')

    def test_incomplete_or_failed_validation_cannot_activate(self):
        results = self.results()
        with self.assertRaises(ValueError):
            images.activate('test-v1', results[:1])
        results[1]['passed'] = False
        with self.assertRaises(ValueError):
            images.activate('test-v1', results)
        self.assertFalse((self.root / 'data/active-image.json').exists())

    def test_changed_manifest_after_activation_rejected(self):
        images.activate('test-v1', self.results())
        (self.directory / 'manifest.json').write_text(json.dumps({**self.manifest, 'extra': True}))
        with self.assertRaises(ValueError):
            images.resolve_image()

    def test_path_traversal_rejected(self):
        for name in ['../outside', '/absolute', '.', '..', 'image/name']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                images.image_directory(name)


if __name__ == '__main__':
    unittest.main()
