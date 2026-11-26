"""Behavioral capture and executed-test provenance checks; all temporary data stays on D:."""
import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('length_validation', Path(__file__).with_name('run.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ValidationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='seata-go-length-tests-', dir='D:/Temp')
        cls.directory = Path(cls.temporary.name).resolve()
        assert cls.directory.is_relative_to(Path('D:/Temp').resolve())
        cls.work = cls.directory / 'cohort'
        cls.work.mkdir()
        with tarfile.open(runner.c.ARCHIVE) as archive:
            for entry in archive:
                relative = '/'.join(entry.name.split('/')[1:])
                if not entry.isfile() or not (relative.endswith('.go') or relative in ['go.mod', 'go.sum']):
                    continue
                path = (cls.work / relative).resolve()
                assert path.is_relative_to(cls.work.resolve())
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(archive.extractfile(entry).read())
        cls.rows = json.loads((runner.ROOT / 'tests/fixtures/protocol/v1/error_lengths/vectors.json').read_text(encoding='utf-8'))
        cls.test_bytes = (runner.ROOT / 'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes()
        cls.test_path = cls.work / 'pkg/protocol/codec/java_v1_length_compat_test.go'

    @classmethod
    def tearDownClass(cls):
        assert cls.directory.is_relative_to(Path('D:/Temp').resolve())
        cls.temporary.cleanup()

    def capture(self, rows):
        path = self.directory / 'capture.log'
        path.write_bytes(b'Unrelated localized output is not parsed: \xd3\xd0\n' + b''.join(
            b'SEATA_GO_LENGTH ' + json.dumps(row, ensure_ascii=True).encode('ascii') + b'\n' for row in rows))
        return path

    def test_complete_ascii_records_preserve_chinese_semantics(self):
        self.assertEqual(runner.read_rows(self.capture(self.rows)), self.rows)

    def test_missing_and_duplicate_inventory_are_rejected(self):
        for rows in [self.rows[:-1], self.rows[:-1] + [self.rows[0]]]:
            with self.subTest(count=len(rows)), self.assertRaises(RuntimeError):
                runner.read_rows(self.capture(rows))

    def test_corrupt_length_error_and_trailing_bytes_are_rejected(self):
        for position in ['length', 'error', 'trailing']:
            rows = [dict(row) for row in self.rows]
            row = next(r for r in rows if r['name'] == 'Branch_AsciiOne')
            data = bytearray.fromhex(row['serialized_hex'])
            if position == 'length':
                data[4] = 0
            elif position == 'error':
                data[-1] = 8
            else:
                data.append(0)
            row['serialized_hex'] = data.hex()
            with self.subTest(position=position), self.assertRaises(RuntimeError):
                runner.read_rows(self.capture(rows))

    def test_codepage_dependent_record_is_rejected(self):
        path = self.directory / 'nonascii.log'
        path.write_bytes(b''.join(b'SEATA_GO_LENGTH ' + json.dumps(row, ensure_ascii=False).encode('gbk') + b'\n' for row in self.rows))
        with self.assertRaises(UnicodeDecodeError):
            runner.read_rows(path)

    def test_unchanged_cohort_includes_all_original_sources(self):
        self.test_path.write_bytes(self.test_bytes)
        rows = runner.audit_cohort(self.work, {}, self.test_bytes)
        self.assertEqual(len(rows), 714)
        self.assertTrue(all(r['original_sha256'] == r['actual_sha256'] for r in rows))

    def test_actual_cohort_test_drift_is_rejected(self):
        self.test_path.write_bytes(self.test_bytes + b'\n// simulated concurrent edit\n')
        try:
            with self.assertRaises(RuntimeError):
                runner.audit_cohort(self.work, {}, self.test_bytes)
        finally:
            self.test_path.write_bytes(self.test_bytes)


if __name__ == '__main__':
    unittest.main()
