"""Full mode must cover both application groups and preserve failures."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('suite', Path(__file__).with_name('suite.py'))
suite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(suite)


class SuiteTests(unittest.TestCase):
    def test_full_continues_after_campaign_failure_and_remains_failed(self):
        with tempfile.TemporaryDirectory() as directory:
            h = SimpleNamespace(out=Path(directory), checks=[], logged=lambda *a, **k: None)
            with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_CAMPAIGN': 'full'}), \
                 patch.object(suite, 'campaign', side_effect=RuntimeError('Firefox failed')), \
                 patch.object(suite, 'core') as core:
                with self.assertRaisesRegex(RuntimeError, 'Firefox failed'):
                    suite.run(h)
                core.assert_called_once_with(h)
                self.assertFalse(h.checks[0]['passed'])

    def test_default_runs_campaign_then_core(self):
        order = []
        with tempfile.TemporaryDirectory() as directory:
            h = SimpleNamespace(out=Path(directory), checks=[], logged=lambda *a, **k: None)
            with patch.dict(suite.os.environ, {}, clear=True), \
                 patch.object(suite, 'campaign', side_effect=lambda h: order.append('campaign')), \
                 patch.object(suite, 'core', side_effect=lambda h: order.append('core')):
                suite.run(h)
        self.assertEqual(order, ['campaign', 'core'])

    def test_standard_and_stress_timings_are_explicit(self):
        for profile, expected in [('standard', (90, 500)), ('stress', (15, 250))]:
            with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_TIMING': profile}):
                self.assertEqual(suite.timing(), expected)
        with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_TIMING': 'typo'}):
            with self.assertRaises(ValueError):
                suite.timing()

    def test_repeat_campaign_keeps_failed_attempts(self):
        calls = []
        def logged(name, *args, **kwargs):
            calls.append(name)
            return b'{"application":"test","passed":false}' if name == 'firefox' else b'{"application":"test","passed":true}'
        with tempfile.TemporaryDirectory() as directory:
            h = SimpleNamespace(out=Path(directory), checks=[], logged=logged)
            with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_CAMPAIGN_REPEATS': '2'}):
                with self.assertRaises(RuntimeError):
                    suite.campaign(h)
            self.assertEqual([c['name'] for c in h.checks], ['firefox', 'zed', 'firefox-2', 'zed-2'])
            self.assertFalse(h.checks[0]['passed'])
            self.assertTrue(h.checks[2]['passed'])
            self.assertTrue((h.out / 'firefox-result.json').exists())
            self.assertTrue((h.out / 'firefox-2-result.json').exists())

    def test_undo_stress_routes_to_the_focused_fixture(self):
        with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_CAMPAIGN': 'undo-stress'}), \
             patch.object(suite, 'campaign') as campaign:
            suite.run(None)
            campaign.assert_called_once_with(None, fixture='undo')

    def test_campaign_repeats_are_bounded(self):
        for value in ('0', '11', 'bad'):
            with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_CAMPAIGN_REPEATS': value}):
                with self.assertRaises(ValueError):
                    suite.campaign_repeats()

    def test_unknown_selection_does_not_silently_run_core(self):
        with patch.dict(suite.os.environ, {'SNIPEXPAND_VM_CAMPAIGN': 'typo'}):
            with self.assertRaises(ValueError):
                suite.run(None)


if __name__ == '__main__':
    unittest.main()
