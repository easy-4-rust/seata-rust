import copy
import json
from pathlib import Path
import tempfile
import unittest

try:
    import collect
except ImportError:
    collect = None

REPO = Path(__file__).resolve().parents[2]


class HandlerCaptureTest(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(collect, 'Runtime response collector is missing')
        root = REPO / 'target'
        root.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix='handler_capture_', dir=root)
        self.base = Path(self.temp.name).resolve()
        self.assertTrue(self.base.is_relative_to(root.resolve()))
        self.addCleanup(self.temp.cleanup)
        self.log = self.base / 'java.log'
        self.destination = self.base / 'fixtures'
        self.events = []
        for case_id, fields in collect.expected_cases().items():
            row = {'format_version':1, 'case_id':case_id, **fields}
            if 'issue' not in fields:
                row['payload_hex'] = collect.encode_expected(fields).hex()
            self.events.append(row)

    def write(self, events=None, success=True):
        lines = []
        for row in self.events if events is None else events:
            prefix = 'SEATA_HANDLER_UNENCODABLE ' if 'issue' in row else 'SEATA_HANDLER_WIRE '
            lines.append(prefix + json.dumps(row, separators=(',', ':')))
        lines += ['Tests run: 135, Failures: 0, Errors: 0, Skipped: 0']
        if success: lines.append('BUILD SUCCESS')
        self.log.write_text('\n'.join(lines) + '\n', encoding='utf-8')

    def test_complete_capture_exports_78_responses_and_separate_unencodable_case(self):
        self.write()
        manifest = collect.extract(self.log, self.destination)
        self.assertEqual(manifest['case_count'], 79)
        self.assertEqual(manifest['sample_count'], 78)
        self.assertEqual(manifest['unencodable_cases'], [{'case_id':'report/null_failure','issue':'JAVA-V1-REPORT-NULL-001'}])
        self.assertEqual(len(list(self.destination.glob('*.bin'))), 78)
        self.assertEqual(len(list(self.destination.glob('*.expect'))), 78)
        self.assertEqual((self.destination/'matrix_lock_none.expect').read_text().split('\t')[7], '1')
        before = {p.name:p.read_bytes() for p in self.destination.iterdir()}
        self.assertEqual(collect.extract(self.log, self.destination), manifest)
        self.assertEqual(before, {p.name:p.read_bytes() for p in self.destination.iterdir()})

    def test_missing_duplicate_unknown_cases_or_failed_build_cannot_export(self):
        for mode in ['missing', 'duplicate', 'unknown', 'failed_build']:
            rows = copy.deepcopy(self.events)
            if mode == 'missing': rows.pop()
            elif mode == 'duplicate': rows.append(rows[0])
            elif mode == 'unknown': rows[0]['case_id'] = '../outside'
            self.write(rows, success=mode != 'failed_build')
            with self.assertRaises(ValueError): collect.extract(self.log, self.destination)
            self.assertFalse(self.destination.exists(), mode)

    def test_semantic_or_payload_mutation_is_rejected_before_writing(self):
        for field in ['result_code', 'payload_hex', 'exception_name']:
            rows = copy.deepcopy(self.events)
            row = next(r for r in rows if r['case_id'] == 'matrix/BEGIN/TRANSACTION')
            if field == 'result_code': row[field] = 1
            elif field == 'payload_hex': row[field] = 'ffff' + row[field][4:]
            else: row[field] = 'UnrecognizedAlias'
            self.write(rows)
            with self.assertRaises(ValueError): collect.extract(self.log, self.destination)
            self.assertFalse(self.destination.exists())

    def test_unknown_version_bool_as_integer_and_malformed_event_are_rejected(self):
        for mode in ['version', 'bool', 'json']:
            rows = copy.deepcopy(self.events)
            row = next(r for r in rows if 'payload_hex' in r)
            if mode == 'version': row['format_version'] = 2
            elif mode == 'bool': row['result_code'] = True
            self.write(rows)
            if mode == 'json': self.log.write_text(self.log.read_text() + 'SEATA_HANDLER_WIRE {bad json}\n')
            with self.assertRaises(ValueError): collect.extract(self.log, self.destination)
            self.assertFalse(self.destination.exists())

    def test_differing_existing_artifact_is_preserved_without_partial_writes(self):
        self.write()
        self.destination.mkdir()
        saved = self.destination/'matrix_begin_none.bin'
        saved.write_bytes(b'user artifact')
        with self.assertRaises(ValueError): collect.extract(self.log, self.destination)
        self.assertEqual(list(self.destination.iterdir()), [saved])
        self.assertEqual(saved.read_bytes(), b'user artifact')

    def test_unencodable_event_cannot_be_replaced_by_fabricated_response(self):
        rows = copy.deepcopy(self.events)
        row = next(r for r in rows if r['case_id'] == 'report/null_failure')
        row['payload_hex'] = '0012010000'
        self.write(rows)
        with self.assertRaises(ValueError): collect.extract(self.log, self.destination)
        self.assertFalse(self.destination.exists())


if __name__ == '__main__': unittest.main()
