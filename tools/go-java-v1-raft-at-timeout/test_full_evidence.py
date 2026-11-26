"""Corrupt a consistent D: unit copy of live evidence; never alter the original run."""
import contextlib,importlib.util,json,os,shutil,tempfile,types,unittest
from pathlib import Path
import evidence

class FullEvidenceGuards(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  source=Path(os.environ['SEATA_RAFT_AT_EVIDENCE']);cls.temporary=tempfile.TemporaryDirectory(prefix='raft-full-unit-',dir='D:/Temp');cls.root=Path(cls.temporary.name);assert cls.root.resolve().is_relative_to(Path('D:/Temp').resolve())
  def translate(value):
   if isinstance(value,str):return value.replace(source.as_posix(),cls.root.as_posix()).replace(str(source),str(cls.root))
   if isinstance(value,list):return [translate(v) for v in value]
   if isinstance(value,dict):return {k:translate(v) for k,v in value.items()}
   return value
  for path in source.rglob('*'):
   if not path.is_file():continue
   relative=path.relative_to(source);top=relative.parts[0]
   include=len(relative.parts)==1 and path.suffix in ['.json','.tsv','.txt','.yml','.jfr'] or top.startswith(('node_','snapshot-data-')) or top in ['initial','operator_loss','operator_replay','compiled']
   if not include:continue
   target=cls.root/relative;target.parent.mkdir(parents=True,exist_ok=True)
   if path.suffix=='.json':target.write_text(json.dumps(translate(json.loads(path.read_bytes()))),encoding='utf-8')
   elif path.suffix=='.log':target.write_text(path.read_text(encoding='utf-8',errors='replace').replace(source.as_posix(),cls.root.as_posix()),encoding='utf-8')
   else:shutil.copyfile(path,target)
  cls.validator=evidence
  if os.environ.get('SEATA_RAFT_LEGACY_VALIDATOR'):
   legacy=types.ModuleType('legacy_full_evidence');legacy.__file__=evidence.__file__;exec(compile(Path(os.environ['SEATA_RAFT_LEGACY_VALIDATOR']).read_bytes(),evidence.__file__,'exec'),legacy.__dict__);cls.validator=legacy
 @classmethod
 def tearDownClass(cls):cls.temporary.cleanup()
 def read(self):return self.validator.derive(self.root)
 @contextlib.contextmanager
 def change(self,name,action):
  path=self.root/name;raw=path.read_bytes();value=json.loads(raw);action(value);path.write_text(json.dumps(value),encoding='utf-8')
  try:yield
  finally:path.write_bytes(raw)
 def test_actual_consistent_unit_copy(self):self.read()
 def test_route_must_bind_confirmed_leader_term(self):
  with self.change('operator_loss/route.json',lambda v:v.update(term=v['term']+1)):
   with self.assertRaises(Exception):self.read()
 def test_business_after_sample_must_remain_same_term(self):
  with self.change('business-leader-after-lost.json',lambda v:v.update(term=v['term']+1)):
   with self.assertRaises(Exception):self.read()
 def test_completed_snapshot_cannot_be_missing(self):
  path=self.root/'snapshot-timeout-node_0.json';raw=path.read_bytes();path.unlink()
  try:
   with self.assertRaises(Exception):self.read()
  finally:path.write_bytes(raw)
 def test_archived_snapshot_bytes_cannot_be_replaced(self):
  path=self.root/'snapshot-data-timeout-node_0/session';raw=path.read_bytes();path.write_bytes(raw+b'x')
  try:
   with self.assertRaises(Exception):self.read()
  finally:path.write_bytes(raw)
 def test_snapshot_response_requires_actual_generation(self):
  with self.change('snapshot-files-timeout-node_0.json',lambda v:v.update(pid=v['pid']+1)):
   with self.assertRaises(Exception):self.read()
 def test_restart_must_consume_previous_snapshot(self):
  with self.change('resume-first.json',lambda v:v['files'][0].update(sha256='0'*64)):
   with self.assertRaises(Exception):self.read()
 def test_jfr_window_cannot_be_moved_into_future(self):
  with self.change('reload-election_one.json',lambda v:v.update(boundary='9999-01-01T00:00:00+00:00')):
   with self.assertRaises(Exception):self.read()
 def test_exception_json_must_reproduce_actual_jfr(self):
  with self.change('election_one-exceptions.json',lambda v:v['recording']['events'].append(dict(tampered=True))):
   with self.assertRaises(Exception):self.read()

if __name__=='__main__':unittest.main()
