"""Capture identity and complete inherited-cohort audit regressions; all temporary files stay on D:."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('merge_validation',Path(__file__).with_name('run.py'))
runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)


class ValidationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary=tempfile.TemporaryDirectory(prefix='seata-merge-validation-',dir='D:/Temp')
        cls.directory=Path(cls.temporary.name).resolve();assert cls.directory.is_relative_to(Path('D:/Temp').resolve())
        # Reuse the already executed owned candidate, in a separate disposable copy.
        latest=json.loads((runner.ROOT/'docs/phase-0/evidence/local-go-java-merge-profile/latest_run.json').read_text(encoding='utf-8'))
        summary=json.loads((Path(latest['evidence_directory'])/'summary.json').read_text(encoding='utf-8'))
        cls.work=cls.directory/'cohort';shutil.copytree(summary['cohort'],cls.work)
        cls.rows=json.loads((runner.ROOT/'tests/fixtures/protocol/v1/merge_profile/vectors.json').read_text(encoding='utf-8'))

    @classmethod
    def tearDownClass(cls):
        assert cls.directory.is_relative_to(Path('D:/Temp').resolve());cls.temporary.cleanup()

    def log(self,rows,prefix):
        path=self.directory/'capture.log';path.write_bytes(b'Unrelated localized output \xd3\xd0\n'+b''.join(prefix+json.dumps(row).encode('ascii')+b'\n' for row in rows));return path

    def outputs(self):
        return [dict(name=name,fields_match=True,bytes_match=True,pointer_match=True,caller_unchanged=True,roundtrip_match=True,java_semantics_match=True) for name in sorted(runner.NAMES|runner.ORIGINAL)]

    def test_complete_capture_and_candidate_closure(self):
        self.assertEqual(len(runner.read_rows(self.log(self.rows,b'SEATA_MERGE_PROFILE '))),38)
        audit=runner.audit_candidate(self.work);self.assertEqual(len(audit['original_members']),714);self.assertEqual(len(audit['profile_files']),5)

    def test_missing_and_duplicate_java_cases_are_rejected(self):
        for rows in [self.rows[:-1],self.rows[:-1]+[self.rows[0]]]:
            with self.subTest(count=len(rows)),self.assertRaises(RuntimeError):runner.read_rows(self.log(rows,b'SEATA_MERGE_PROFILE '))

    def test_sealed_lengths_and_absent_version_semantics_are_checked(self):
        for mode in ['length','getter','reencoded']:
            rows=[dict(row) for row in self.rows];row=next(row for row in rows if row['name']=='AbsentTMVersion')
            if mode=='length':data=bytearray.fromhex(row['serialized_hex']);data[5]^=1;row['serialized_hex']=data.hex()
            elif mode=='getter':row['java_effective_version']=''
            else:row['java_reencoded_hex']=row['serialized_hex']
            with self.subTest(mode=mode),self.assertRaises(RuntimeError):runner.read_rows(self.log(rows,b'SEATA_MERGE_PROFILE '))

    def test_result_inventory_requires_all44_unique_cases(self):
        rows=self.outputs();self.assertEqual(len(runner.results(self.log(rows,b'SEATA_MERGE_RESULT '))),44)
        for invalid in [rows[:-1],rows[:-1]+[rows[0]]]:
            with self.subTest(count=len(invalid)),self.assertRaises(RuntimeError):runner.results(self.log(invalid,b'SEATA_MERGE_RESULT '))

    def test_semantics_pointer_and_caller_checks_cannot_be_omitted(self):
        for key in ['java_semantics_match','pointer_match','caller_unchanged']:
            rows=self.outputs();rows[0][key]=False
            with self.subTest(key=key),self.assertRaises(RuntimeError):runner.results(self.log(rows,b'SEATA_MERGE_RESULT '))

    def test_inherited_length_and_exception_implementation_drift_is_rejected(self):
        for relative in ['pkg/protocol/codec/global_lock_query_resp_codec.go','pkg/protocol/codec/java_v1_response_profile.go']:
            path=self.work/relative;original=path.read_bytes();path.write_bytes(original+b'\n// concurrent drift\n')
            try:
                with self.subTest(path=relative),self.assertRaises(RuntimeError):runner.audit_candidate(self.work)
            finally:path.write_bytes(original)

    def test_unexpected_source_and_changed_inherited_test_are_rejected(self):
        extra=self.work/'pkg/protocol/codec/unexpected.go';extra.write_text('package codec\n',encoding='utf-8')
        try:
            with self.assertRaises(RuntimeError):runner.audit_candidate(self.work)
        finally:extra.unlink()
        path=self.work/'pkg/protocol/codec/java_v1_response_profile_test.go';original=path.read_bytes();path.write_bytes(original+b'\n// changed test\n')
        try:
            with self.assertRaises(RuntimeError):runner.audit_candidate(self.work)
        finally:path.write_bytes(original)


if __name__=='__main__':unittest.main()
