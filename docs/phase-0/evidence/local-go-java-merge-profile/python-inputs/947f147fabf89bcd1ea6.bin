"""Capture/case-identity behavior checks using actual Java fixture bytes; D: temporary files only."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('exception_validation',Path(__file__).with_name('run.py'))
runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)


class ValidationTest(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory(prefix='seata-exception-validation-',dir='D:/Temp')
        self.directory=Path(self.temporary.name).resolve()
        self.assertTrue(self.directory.is_relative_to(Path('D:/Temp').resolve()))
        self.rows=json.loads((runner.ROOT/'tests/fixtures/protocol/v1/exception_profile/vectors.json').read_text(encoding='utf-8'))

    def tearDown(self):
        assert self.directory.is_relative_to(Path('D:/Temp').resolve())
        self.temporary.cleanup()

    def log(self,rows,prefix):
        path=self.directory/'capture.log'
        path.write_bytes(b'Unrelated localized Maven output: \xd3\xd0\n'+b''.join(prefix+json.dumps(row,ensure_ascii=True).encode('ascii')+b'\n' for row in rows))
        return path

    def outputs(self,bad=()):
        return [dict(name=name,encode_match=name not in bad,decode_match=name not in bad,reencode_match=name not in bad,
                     pointer_checked=True,pointer_match=True) for name in sorted(runner.EXPECTED)]

    def test_complete_actual_java_capture_is_accepted(self):
        self.assertEqual(runner.read_rows(self.log(self.rows,b'SEATA_EXCEPTION_PROFILE ')),self.rows)

    def test_missing_duplicate_capture_cases_are_rejected(self):
        for rows in [self.rows[:-1],self.rows[:-1]+[self.rows[0]]]:
            with self.subTest(size=len(rows)),self.assertRaises(RuntimeError):
                runner.read_rows(self.log(rows,b'SEATA_EXCEPTION_PROFILE '))

    def test_wire_type_error_and_message_must_match_metadata(self):
        for position in [1,5,-1]:
            rows=[dict(row) for row in self.rows]
            row=next(row for row in rows if row['name']=='14_Unknown')
            wire=bytearray.fromhex(row['serialized_hex']);wire[position]^=1;row['serialized_hex']=wire.hex()
            with self.subTest(position=position),self.assertRaises(RuntimeError):
                runner.read_rows(self.log(rows,b'SEATA_EXCEPTION_PROFILE '))

    def test_result_inventory_rejects_missing_and_duplicates(self):
        rows=self.outputs()
        for invalid in [rows[:-1],rows[:-1]+[rows[0]]]:
            with self.subTest(size=len(invalid)),self.assertRaises(RuntimeError):
                runner.results(self.log(invalid,b'SEATA_EXCEPTION_RESULT '),set())

    def test_same_divergence_count_with_wrong_identity_is_rejected(self):
        expected={'2_NotRaftLeader'}
        rows=self.outputs({'2_LockKeyConflictFailFast'})
        with self.assertRaises(RuntimeError):
            runner.results(self.log(rows,b'SEATA_EXCEPTION_RESULT '),expected)

    def test_profile_pointer_checks_are_required(self):
        rows=self.outputs()
        self.assertEqual(len(runner.results(self.log(rows,b'SEATA_EXCEPTION_RESULT '),set())),250)
        for key in ['pointer_checked','pointer_match']:
            invalid=[dict(row) for row in rows];invalid[0][key]=False
            with self.subTest(key=key),self.assertRaises(RuntimeError):
                runner.results(self.log(invalid,b'SEATA_EXCEPTION_RESULT '),set())


if __name__=='__main__':unittest.main()
