import json
from pathlib import Path
import tempfile
import unittest

try:
    from export_fixtures import export_fixtures
except ImportError:
    export_fixtures = None

REPO = Path(__file__).resolve().parents[2]


class FailureFixtureExportTest(unittest.TestCase):
    def setUp(self):
        root = REPO / 'target'
        root.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix='failure_export_', dir=root)
        self.base = Path(self.temp.name).resolve()
        self.assertTrue(self.base.is_relative_to(root.resolve()))
        self.addCleanup(self.temp.cleanup)
        self.source = self.base / 'input.json'
        self.rows = json.loads((REPO / 'tools/go-failure-variants/expected.json').read_text())
        self.source.write_text(json.dumps(self.rows), encoding='utf-8')
        self.destination = self.base / 'output'

    def export(self):
        self.assertIsNotNone(export_fixtures, 'Failure fixture exporter is missing')
        return export_fixtures(self.source, self.destination)

    def test_exports_distinct_java_and_go_bytes_with_stable_provenance(self):
        result = self.export()
        self.assertEqual(result['sample_count'], 7)
        self.assertEqual(result['compatible_cases'], 4)
        self.assertEqual(result['open_divergences'], 3)
        self.assertEqual(len(list(self.destination.glob('*.bin'))), 14)
        self.assertEqual((self.destination / 'BranchReportResponseFailed.java.bin').read_bytes(),
                         bytes.fromhex('000e00000d7265706f72742064656e69656407'))
        self.assertEqual((self.destination / 'BranchReportResponseFailed.go.bin').read_bytes(),
                         bytes.fromhex('000e000d7265706f72742064656e69656407'))
        before = {p.name: p.read_bytes() for p in self.destination.iterdir()}
        self.assertEqual(self.export(), result)
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.destination.iterdir()})

    def test_invalid_type_prefix_or_compatibility_prevents_any_output(self):
        for mutate in ['prefix', 'classification']:
            rows = json.loads(json.dumps(self.rows))
            if mutate == 'prefix':
                rows[-1]['java_serialized_hex'] = 'ffff' + rows[-1]['java_body_hex']
            else:
                rows[-1]['compatibility'] = 'byte_equal'
            self.source.write_text(json.dumps(rows), encoding='utf-8')
            with self.assertRaises(ValueError): self.export()
            self.assertFalse(self.destination.exists())

    def test_name_collisions_and_path_traversal_prevent_any_output(self):
        for name in ['../escape', self.rows[0]['name'].lower()]:
            self.rows[1]['name'] = name
            self.source.write_text(json.dumps(self.rows), encoding='utf-8')
            with self.assertRaises(ValueError): self.export()
            self.assertFalse(self.destination.exists())

    def test_differing_artifact_is_preserved_without_partial_writes(self):
        self.destination.mkdir()
        saved = self.destination / 'BranchReportResponseFailed.go.bin'
        saved.write_bytes(b'user data')
        with self.assertRaises(ValueError): self.export()
        self.assertEqual(list(self.destination.iterdir()), [saved])
        self.assertEqual(saved.read_bytes(), b'user data')

    def test_crlf_lf_provenance_is_stable(self):
        text = json.dumps(self.rows, indent=2) + '\n'
        self.source.write_bytes(text.replace('\n', '\r\n').encode())
        first = self.export()
        self.source.write_bytes(text.encode())
        self.assertEqual(first, self.export())


if __name__ == '__main__': unittest.main()
