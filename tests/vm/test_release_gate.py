"""The release gate must reject partial, stale, dirty, or altered evidence."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('gate', Path(__file__).with_name('release-check.py'))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class GateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(gate, 'ROOT', self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.commit = 'a' * 40
        self.manifest = {'commit': self.commit, 'status': '', 'source': str(self.root),
                         'sha256': {}, 'harness_sha256': {}}
        for name in ['target/release/snipexpand', 'target/debug/examples/e2e_type',
                     'tests/vm/guest-campaign.py', 'tests/vm/guest-packages.py']:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
            self.manifest['sha256'][name] = gate.digest(path)
        harness = self.root / 'harness.py'
        harness.write_text('guarded harness')
        self.manifest['harness_sha256'][str(harness)] = gate.digest(harness)
        self.result = {'project': 'snipexpand', 'passed': True, 'artifacts': str(self.root),
                       'image': {'sealed': True, 'name': 'test-image', 'manifest_sha256': 'b' * 64},
                       'checks': [{'name': name, 'passed': True, 'event_delay_ms': 90, 'expansion_pause_ms': 500} for name in sorted(gate.REQUIRED)]}
        self.save_manifest()

    def save_manifest(self):
        (self.root / 'payload-manifest.json').write_text(json.dumps(self.manifest))

    def test_complete_evidence_passes(self):
        evidence, _ = gate.validate_result([self.result], self.commit)
        self.assertEqual(evidence, self.root)

    def test_partial_failed_duplicate_and_cleanup_failures_rejected(self):
        for mutate in [lambda r: r['checks'].pop(),
                       lambda r: r['checks'].append(r['checks'][0]),
                       lambda r: r['checks'][0].update(passed=False),
                       lambda r: r.update(passed=False),
                       lambda r: r.update(cleanup_errors=['stop failed']),
                       lambda r: r.update(image=None),
                       lambda r: r['checks'][0].update(event_delay_ms=15)]:
            with self.subTest(mutate=mutate):
                result = copy.deepcopy(self.result)
                mutate(result)
                with self.assertRaises(ValueError):
                    gate.validate_result([result], self.commit)

    def test_stale_commit_dirty_source_and_missing_provenance_rejected(self):
        for change in [{'commit': 'c' * 40}, {'status': ' M src/main.rs'},
                       {'sha256': {}}, {'harness_sha256': {}}, {'source': '/another/checkout'}]:
            with self.subTest(change=change):
                original = copy.deepcopy(self.manifest)
                self.manifest.update(change)
                self.save_manifest()
                with self.assertRaises(ValueError):
                    gate.validate_result([self.result], self.commit)
                self.manifest = original

    def test_changed_binary_or_harness_rejected(self):
        for name in ['target/release/snipexpand', 'harness.py']:
            path = self.root / name
            original = path.read_text()
            path.write_text('changed')
            with self.assertRaises(ValueError):
                gate.validate_result([self.result], self.commit)
            path.write_text(original)

    def test_status_checks_exact_commit_actor_and_latest_outcome(self):
        good = {'id': 1, 'context': gate.CONTEXT, 'state': 'success',
                'creator': {'login': 'owner'}, 'description': gate.PREFIX + 'd' * 64}
        gate.check_status({'sha': self.commit, 'statuses': [good]}, self.commit, 'owner')
        invalid = [({'sha': 'e' * 40, 'statuses': [good]}),
                   {'sha': self.commit, 'statuses': []}]
        for change in [{'state': 'pending'}, {'state': 'failure'}, {'description': 'manual pass'},
                       {'creator': {'login': 'another-person'}}]:
            invalid.append({'sha': self.commit, 'statuses': [good, {**good, **change, 'id': 2}]})
        for data in invalid:
            with self.subTest(data=data), self.assertRaises(ValueError):
                gate.check_status(data, self.commit, 'owner')

    def test_release_run_overrides_stress_and_repeat_environment(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(gate, 'clean_commit', return_value=self.commit), \
             patch.dict(gate.os.environ, {'SNIPEXPAND_VM_CAMPAIGN': 'undo-stress',
                        'SNIPEXPAND_VM_TIMING': 'stress', 'SNIPEXPAND_VM_CAMPAIGN_REPEATS': '10'}), \
             patch.object(gate.sp, 'run', side_effect=RuntimeError('stop before VM')) as run:
            with self.assertRaisesRegex(RuntimeError, 'stop before VM'):
                gate.run(SimpleNamespace(output=Path(directory) / "evidence", image=None))
            env = run.call_args.kwargs['env']
            self.assertEqual(env['SNIPEXPAND_VM_CAMPAIGN'], 'full')
            self.assertEqual(env['SNIPEXPAND_VM_TIMING'], 'standard')
            self.assertEqual(env['SNIPEXPAND_VM_CAMPAIGN_REPEATS'], '1')

    def test_dirty_checkout_cannot_publish(self):
        with patch.object(gate, 'git', return_value='?? untracked.rs'):
            with self.assertRaises(ValueError):
                gate.clean_commit()


if __name__ == '__main__':
    unittest.main()
