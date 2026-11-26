"""Real captured AT fixtures plus negative provenance/identity controls; all scratch on D:."""
import copy,importlib.util,json,os,shutil,tempfile,unittest
import hashlib
from pathlib import Path
from unittest.mock import patch
HERE=Path(__file__).resolve().parent;spec=importlib.util.spec_from_file_location('at_verify_test',HERE/'verify.py');verify=importlib.util.module_from_spec(spec);spec.loader.exec_module(verify)
NORMAL=Path(os.environ.get('SEATA_REAL_AT_NORMAL','D:/codex-builds/real-at-20261112_233720_8196c1/evidence'))
RECONNECT=Path(os.environ.get('SEATA_REAL_AT_RECONNECT','D:/codex-builds/real-at-20261112_234433_f3cf23/evidence'))
RECOVERY=Path(os.environ.get('SEATA_REAL_AT_RECOVERY','D:/codex-builds/real-at-20261112_234740_fc4ee5/evidence'))
class VerifyTest(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory(prefix='real-at-verify-',dir='D:/Temp');self.scratch=Path(self.temp.name)
 def tearDown(self):
  assert self.scratch.resolve().is_relative_to(Path('D:/Temp').resolve());self.temp.cleanup()
 def case(self,source):
  target=self.scratch/'case';target.mkdir()
  for name in ['client.log','proxy-events.json','proxy-result.json','database-final.tsv','prepare.log','database-before-restart.tsv','database-after-restart.tsv','restart.json']:
   if (source/name).exists():shutil.copyfile(source/name,target/name)
  if (source/'resumed').exists():
   (target/'resumed').mkdir()
   for name in ['client.log','proxy-events.json','proxy-result.json']:shutil.copyfile(source/'resumed'/name,target/'resumed'/name)
  return target
 def test_actual_normal_and_reconnect(self):
  self.assertEqual(verify.normal(NORMAL,False,strict=False)['actual_callbacks'],2)
  self.assertEqual(verify.normal(RECONNECT,True,strict=False)['connections'],2)
 def test_actual_database_and_process_restart(self):self.assertTrue(verify.recovery(RECOVERY,strict=False)['database_bytes_preserved'])
 def test_wire_metadata_cannot_override_raw_frame(self):
  event=next(e for e in json.loads((RECONNECT/'proxy-events.json').read_bytes()) if e['event']=='wire' and e['payload_type']==12);event['id']+=1
  with self.assertRaises(RuntimeError):verify.decode(event)
 def test_reported_branch_must_match_original_tc_id(self):
  target=self.case(RECONNECT);data=(target/'client.log').read_bytes();prefix=b'REAL_AT_PASS ';old=next(line for line in data.splitlines() if line.startswith(prefix));proof=json.loads(old[len(prefix):]);proof['rollback']['branch_id']+=1;(target/'client.log').write_bytes(data.replace(old,prefix+json.dumps(proof).encode()))
  with self.assertRaises(RuntimeError):verify.normal(target,True,strict=False)
 def test_independent_business_row_must_match(self):
  target=self.case(RECONNECT);(target/'database-final.tsv').write_text('1\t999\n',encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.normal(target,True,strict=False)
 def test_replayed_reply_must_keep_original_request_id(self):
  target=self.case(RECONNECT);events=json.loads((target/'proxy-events.json').read_bytes());row=next(e for e in events if e['event']=='duplicate-rm-reply');frame=bytearray.fromhex(row['frame_hex']);frame[12:16]=(123456).to_bytes(4,'big');row['frame_hex']=frame.hex();(target/'proxy-events.json').write_text(json.dumps(events),encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.normal(target,True,strict=False)
 def test_persisted_undo_bytes_must_survive_database_restart(self):
  target=self.case(RECOVERY);(target/'database-after-restart.tsv').write_bytes((target/'database-after-restart.tsv').read_bytes()+b'changed\n')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_equal_empty_independent_sql_is_rejected(self):
  target=self.case(RECOVERY)
  for name in ['database-before-restart.tsv','database-after-restart.tsv']:(target/name).write_bytes(b'')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_equal_wrong_independent_sql_is_rejected(self):
  target=self.case(RECOVERY)
  for name in ['database-before-restart.tsv','database-after-restart.tsv']:(target/name).write_bytes(b'1\t901\nwrong\t1\t0\t00\n')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_recovered_rm_registration_is_required(self):
  target=self.case(RECOVERY);path=target/'resumed/proxy-events.json';events=json.loads(path.read_bytes());events=[e for e in events if not(e['event']=='wire' and e['payload_type'] in [103,104])];path.write_text(json.dumps(events),encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_recovered_reply_id_must_match_tc_request(self):
  target=self.case(RECOVERY);path=target/'resumed/proxy-events.json';events=json.loads(path.read_bytes());e=next(e for e in events if e['event']=='wire' and e['payload_type']==6);data=bytearray.fromhex(e['frame_hex']);data[12:16]=(123456).to_bytes(4,'big');e.update(frame_hex=data.hex(),id=123456,sha256=hashlib.sha256(data).hexdigest());path.write_text(json.dumps(events),encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_unexpected_recovery_proxy_error_is_rejected(self):
  target=self.case(RECOVERY);path=target/'resumed/proxy-result.json';state=json.loads(path.read_bytes());state['errors']=['unexpected protocol failure'];path.write_text(json.dumps(state),encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.recovery(target,strict=False)
 def test_rm_ack_must_precede_tc_callback(self):
  target=self.case(RECONNECT);path=target/'proxy-events.json';events=json.loads(path.read_bytes());callback=next(e for e in events if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==5)
  for e in events:
   if e['event']=='wire' and e['payload_type']==104 and e['connection']==callback['connection']:e['sequence']=callback['sequence']+100
  path.write_text(json.dumps(events),encoding='utf-8')
  with self.assertRaises(RuntimeError):verify.normal(target,True,strict=False)
class OverlayTest(unittest.TestCase):
 def setUp(self):
  spec=importlib.util.spec_from_file_location('real_at_overlay_test',HERE/'overlay.py');self.overlay=importlib.util.module_from_spec(spec);spec.loader.exec_module(self.overlay)
  self.parent=HERE.parents[1]/'docs/phase-0/evidence/local-java-preface-candidate/run_20261112_221159_a16eafb1'
 def test_certified_overlay_is_accepted(self):self.assertEqual(len(self.overlay.verify(self.parent)),10)
 def test_modified_class_at_same_loader_path_is_rejected(self):
  actual=Path.read_bytes;target=next((self.parent/'overlay').rglob('*.class'))
  def altered(path):return actual(path)+b'changed class' if path==target else actual(path)
  with patch.object(Path,'read_bytes',altered),self.assertRaises(RuntimeError):self.overlay.verify(self.parent)
 def test_modified_parent_inventory_is_rejected(self):
  actual=Path.read_bytes;target=self.parent/'overlay-manifest.json'
  def altered(path):return actual(path)+b' ' if path==target else actual(path)
  with patch.object(Path,'read_bytes',altered),self.assertRaises(RuntimeError):self.overlay.verify(self.parent)
if __name__=='__main__':unittest.main()
