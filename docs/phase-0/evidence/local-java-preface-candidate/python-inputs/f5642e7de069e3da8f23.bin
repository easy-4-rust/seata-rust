"""SDK candidate closure and real-loop proof guard regressions; scratch stays on D:."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('sdk_validation',Path(__file__).with_name('run.py'));r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)

class ValidationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='seata-sdk-validation-',dir='D:/Temp');cls.directory=Path(cls.temp.name).resolve()
        if os.environ.get('SEATA_SDK_COHORT'):
            source=Path(os.environ['SEATA_SDK_COHORT']);evidence=Path(os.environ['SEATA_SDK_LIVE_EVIDENCE'])
        else:
            latest=json.loads((r.ROOT/'docs/phase-0/evidence/local-go-java-sdk-profile/latest_run.json').read_bytes());out=Path(latest['evidence_directory']);source=Path(json.loads((out/'summary.json').read_bytes())['cohort']);evidence=out/'sdk-live'
        cls.work=cls.directory/'cohort';shutil.copytree(source,cls.work)
        cls.live=cls.directory/'live';cls.live.mkdir()
        cls.live_inputs={name:(evidence/name).read_bytes() for name in ['proxy-events.json','proxy-result.json','tc-stdout.log']}
        cls.proof=json.loads((evidence/'result.json').read_bytes())['proof']
        cls.reset_live()
    @classmethod
    def reset_live(cls):
        for name,data in cls.live_inputs.items():(cls.live/name).write_bytes(data)
    @classmethod
    def tearDownClass(cls):
        assert cls.directory.is_relative_to(Path('D:/Temp').resolve());cls.temp.cleanup()
    def test_complete_candidate_and_closed_auxiliary_inputs(self):
        audit=r.audit_candidate(self.work);self.assertEqual(len(audit['original_members']),714);self.assertEqual(len(audit['candidate_files']),20);self.assertEqual(len(audit['auxiliary']),12)
    def test_inherited_implementation_drift_is_rejected(self):
        path=self.work/'pkg/protocol/codec/global_lock_query_resp_codec.go';old=path.read_bytes();path.write_bytes(old+b'\n// drift\n')
        try:
            with self.assertRaises(RuntimeError):r.audit_candidate(self.work)
        finally:path.write_bytes(old)
    def test_stale_editable_sdk_template_is_rejected(self):
        generated=r.prep.files();generated['pkg/remoting/getty/java_v1_reply_plan.go']+=b'\n// unbuilt edit\n'
        with patch.object(r.prep,'files',return_value=generated),self.assertRaisesRegex(RuntimeError,'Editable/generated'):r.audit_candidate(self.work)
    def test_executed_sdk_test_drift_is_rejected(self):
        path=self.work/'pkg/remoting/getty/sdk_export_test.go';old=path.read_bytes();path.write_bytes(old+b'\n// changed test binding\n')
        try:
            with self.assertRaises(RuntimeError):r.audit_candidate(self.work)
        finally:path.write_bytes(old)
    def test_unexpected_go_source_is_rejected(self):
        path=self.work/'pkg/remoting/getty/unexpected.go';path.write_text('package getty\n',encoding='utf-8')
        try:
            with self.assertRaises(RuntimeError):r.audit_candidate(self.work)
        finally:path.unlink()
    def test_fault_mutations_have_exact_lengths_types_and_ids(self):
        body=b'\x00\x3c'+(12).to_bytes(4,'big')+b'\x00\x02'+b'\x00\x10\x01\x00\x01'*2
        frame=bytes.fromhex('dada0100000022001001010000000007')+body
        short=r.live.proxy.mutate(frame,'short-result');self.assertEqual(len(short),29);self.assertEqual(int.from_bytes(short[3:7],'big'),29);self.assertEqual(short[22:24],b'\x00\x01')
        changed=r.live.proxy.mutate(frame,'wrong-child-type');self.assertEqual(changed[29:31],b'\x00\x08')
        foreign=r.live.proxy.mutate(frame,'foreign-id');self.assertEqual(int.from_bytes(foreign[12:16],'big'),10007)
    def test_fault_mutations_require_actual_expected_java_shape(self):
        frame=bytes.fromhex('dada0100000010001001010000000007')
        for mode in ['short-result','wrong-child-type','unrecognized']:
            with self.subTest(mode=mode),self.assertRaises(RuntimeError):r.live.proxy.mutate(frame,mode)
    def test_live_inventory_cannot_omit_or_duplicate_actual_requests(self):
        self.reset_live();self.assertEqual(r.live.verify(self.live,self.proof)['requests'],17)
        events=json.loads(self.live_inputs['proxy-events.json']);index=next(i for i,row in enumerate(events) if row['event']=='wire' and row['direction']=='sdk-to-tc' and row['kind']==0)
        for rows in [events[:index]+events[index+1:],events+[events[index]]]:
            (self.live/'proxy-events.json').write_text(json.dumps(rows),encoding='utf-8')
            with self.assertRaises(RuntimeError):r.live.verify(self.live,self.proof)
        self.reset_live()
    def test_live_proof_cannot_claim_resends_or_omit_reconnect_and_unknowns(self):
        self.reset_live()
        for key,value in [('transaction_resends',1),('reconnects',0),('typed_unknown_replies',0),('owned_futures_remaining',1)]:
            proof=dict(self.proof);proof[key]=value
            with self.subTest(key=key),self.assertRaises(RuntimeError):r.live.verify(self.live,proof)

if __name__=='__main__':unittest.main()
