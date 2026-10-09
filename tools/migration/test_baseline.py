import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from baseline import ROOT, inventory, run_process, summarize_tests


class EvidenceTests(unittest.TestCase):
    def test_compiler_failure_is_not_reported_as_passing_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'tests.jsonl'
            p.write_text(json.dumps({'Action':'fail','Package':'game'})+'\n')
            result = summarize_tests(p)
        self.assertEqual(result['failed_packages'], ['game'])
        self.assertEqual(result['test_events']['pass'], 0)

    def test_skipped_database_and_parent_subtest_failures_are_explicit(self):
        events = [
            {'Action':'skip','Package':'account','Test':'TestConnect'},
            {'Action':'pass','Package':'vehicle','Test':'TestDrive/buggy'},
            {'Action':'fail','Package':'vehicle','Test':'TestDrive/bike'},
            {'Action':'fail','Package':'vehicle','Test':'TestDrive'},
            {'Action':'fail','Package':'vehicle'},
        ]
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'tests.jsonl'
            p.write_text('\n'.join(json.dumps(e) for e in events)+'\ncompiler diagnostic\n')
            result = summarize_tests(p)
        self.assertEqual(result['test_events'], {'pass':1,'fail':2,'skip':1})
        self.assertEqual(result['skipped_tests'], ['account/TestConnect'])
        self.assertEqual(result['non_json_lines'], 1)

    def test_inventory_excludes_generated_dependency_sources(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root/'build/deps').mkdir(parents=True)
            (root/'build/deps/foreign_test.go').write_text('func TestForeign(t *testing.T) {}')
            (root/'main_test.go').write_text('//go:build !js\nfunc TestLocal(t *testing.T) {}\nfunc BenchmarkLocal(b *testing.B) {}')
            result = inventory(root)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['tests'], ['TestLocal'])
        self.assertEqual(result[0]['build_tags'], ['!js'])

    def test_commands_have_a_bounded_runtime(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            run_process([sys.executable, '-c', 'import time; time.sleep(30)'], timeout=0.1)

    def test_acceptance_scenarios_resolve_to_existing_sources(self):
        scenarios = json.loads((ROOT/'tools/migration/scenarios.json').read_text())['scenarios']
        ids = [s['id'] for s in scenarios]
        self.assertEqual(len(ids),len(set(ids)))
        for scenario in scenarios:
            with self.subTest(scenario=scenario['id']):
                for pattern in scenario['source_globs']:
                    self.assertTrue(list(ROOT.glob(pattern)), pattern)
                for field in ('setup_and_inputs','expected','comparison','evidence_directory'):
                    self.assertTrue(scenario[field])


if __name__ == '__main__':
    unittest.main()
