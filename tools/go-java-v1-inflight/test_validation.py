"""Reject stale templates, unlisted Go inputs and inconsistent patch dependencies without edits."""
import copy
import importlib.util
import json
import os
import threading
from types import SimpleNamespace
from pathlib import Path
import unittest
from unittest.mock import patch
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('inflight_unit_audit',HERE/'audit.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
WORK=Path(os.environ.get('SEATA_INFLIGHT_COHORT','D:/codex-builds/seata-go-inflight-20261112'))
class InputAuditTest(unittest.TestCase):
 def test_current_candidate_is_accepted(self):
  result=audit.run(WORK);self.assertEqual(len(result['original_members']),714);self.assertEqual(len(result['auxiliary']),13)
 def test_stale_editable_template_is_rejected(self):
  generated=audit.prepare.files();name=next(iter(generated));generated[name]+=b'// edited after patch creation\n'
  with patch.object(audit.prepare,'files',return_value=generated),self.assertRaises(RuntimeError):audit.run(WORK)
 def test_unlisted_go_file_is_rejected(self):
  actual=Path.rglob
  def altered(path,pattern):
   result=list(actual(path,pattern))
   return iter(result+[WORK/'unexpected.go']) if path==WORK and pattern=='*.go' else iter(result)
  with patch.object(Path,'rglob',altered),self.assertRaises(RuntimeError):audit.run(WORK)
 def test_wrong_parent_dependency_hash_is_rejected(self):
  manifest=json.loads(audit.prepare.MANIFEST.read_bytes());name=next(iter(manifest['requires']));manifest['requires'][name]='0'*64
  actual=Path.read_bytes
  def altered(path):return (json.dumps(manifest)+'\n').encode() if path==audit.prepare.MANIFEST else actual(path)
  with patch.object(Path,'read_bytes',altered),self.assertRaises(RuntimeError):audit.run(WORK)

class ProxySendTest(unittest.TestCase):
 def test_parallel_late_frame_and_heartbeat_never_interleave_fragments(self):
  spec=importlib.util.spec_from_file_location('inflight_proxy_send_test',HERE/'proxy.py');proxy=importlib.util.module_from_spec(spec);spec.loader.exec_module(proxy)
  class Stream:
   def __init__(self):self.parts=[]
   def sendall(self,data):self.parts.append(data)
  relay=proxy.InflightProxy.__new__(proxy.InflightProxy);relay.lock=threading.Lock();relay.send_locks={}
  for _ in range(5):
   stream=Stream();connection=SimpleNamespace(number=1,client=stream);gate=threading.Barrier(2);a,b=b'a'*20,b'b'*24
   def send(data):gate.wait();relay.send(connection,stream,data)
   threads=[threading.Thread(target=send,args=(data,)) for data in [a,b]]
   for thread in threads:thread.start()
   for thread in threads:thread.join(timeout=1);self.assertFalse(thread.is_alive())
   self.assertIn(b''.join(stream.parts),[a+b,b+a])
if __name__=='__main__':unittest.main()
