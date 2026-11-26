"""Evidence identity and editable-source binding regressions; scratch files stay on D:."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('getty_validation',Path(__file__).with_name('run.py'))
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)


class ValidationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='seata-getty-validation-',dir='D:/Temp');cls.directory=Path(cls.temp.name).resolve()
        if os.environ.get('SEATA_GETTY_COHORT'):source=Path(os.environ['SEATA_GETTY_COHORT'])
        else:
            latest=json.loads((r.ROOT/'docs/phase-0/evidence/local-go-java-getty-profile/latest_run.json').read_bytes())
            source=Path(json.loads((Path(latest['evidence_directory'])/'summary.json').read_bytes())['cohort'])
        cls.work=cls.directory/'cohort';shutil.copytree(source,cls.work)
        fixtures=r.ROOT/'tests/fixtures/protocol/v1'
        cls.groups=[json.loads(path.read_bytes()) for path in [fixtures/'control_envelopes/vectors.json',fixtures/'merge_profile/vectors.json',fixtures/'exception_profile/vectors.json',r.ROOT/'tools/go-golden-vectors/expected.json']]

    @classmethod
    def tearDownClass(cls):
        assert cls.directory.is_relative_to(Path('D:/Temp').resolve());cls.temp.cleanup()

    def test_frame_count_provenance_and_source_identity(self):
        rows=r.compose(*self.groups);self.assertEqual(len(rows),326)
        self.assertEqual(sum(row['source']=='java-frame' for row in rows),9)
        for index in range(4):
            groups=[list(group) for group in self.groups];groups[index]=groups[index][:-1]
            with self.subTest(index=index),self.assertRaises(RuntimeError):r.compose(*groups)
        groups=[list(group) for group in self.groups];groups[1][0]=groups[1][1]
        with self.assertRaises(RuntimeError):r.compose(*groups)

    def test_complete_candidate_closure(self):
        audit=r.audit_candidate(self.work);self.assertEqual(len(audit['original_members']),714)

    def test_editable_template_mismatch_is_rejected(self):
        generated=r.prep.files();path='pkg/remoting/getty/java_v1_profile_readwriter.go';generated[path]+=b'\n// edited but not rebuilt\n'
        with patch.object(r.prep,'files',return_value=generated),self.assertRaisesRegex(RuntimeError,'Editable/generated'):r.audit_candidate(self.work)

    def test_unexpected_source_is_rejected(self):
        path=self.work/'pkg/remoting/getty/unexpected.go';path.write_text('package getty\n',encoding='utf-8')
        try:
            with self.assertRaises(RuntimeError):r.audit_candidate(self.work)
        finally:path.unlink()

    def test_inherited_and_new_implementation_drift_are_rejected(self):
        for relative in ['pkg/protocol/codec/global_lock_query_resp_codec.go','pkg/protocol/codec/java_v1_response_profile.go','pkg/remoting/getty/readwriter.go']:
            path=self.work/relative;old=path.read_bytes();path.write_bytes(old+b'\n// drift\n')
            try:
                with self.subTest(path=relative),self.assertRaises(RuntimeError):r.audit_candidate(self.work)
            finally:path.write_bytes(old)

    def test_executed_test_drift_is_rejected(self):
        path=self.work/'pkg/remoting/getty/java_v1_profile_readwriter_test.go';old=path.read_bytes();path.write_bytes(old+b'\n// drift\n')
        try:
            with self.assertRaises(RuntimeError):r.audit_candidate(self.work)
        finally:path.write_bytes(old)


if __name__=='__main__':unittest.main()
