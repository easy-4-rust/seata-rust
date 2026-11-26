"""FILE-specific negative controls augment the certified real business rollback guards."""
import copy,importlib.util,unittest
from pathlib import Path
from verify import validate
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('file_timeout_parent_test_data',ROOT/'tools/go-java-v1-real-at-timeout/test_verify.py')
parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
class FileStorageGuards(unittest.TestCase):
 def good(self):
  value=parent.EvidenceGuards().good();journal=[dict(path='28091/root.data',sha256='a'*64,bytes=128)]
  value['file_storage']=dict(mode='file',tables=['accounts','undo_log'],journals=[dict(stage=name,files=copy.deepcopy(journal)) for name in ['initial','operator_loss','operator_replay']],restore_inputs=[dict(stage=name,files=copy.deepcopy(journal)) for name in ['operator_loss','operator_replay']])
  return value
 def test_control(self):validate(self.good())
 def test_database_tc_cannot_claim_file(self):
  value=self.good();value['file_storage']['mode']='db'
  with self.assertRaises(RuntimeError):validate(value)
 def test_tc_sql_tables_rejected(self):
  value=self.good();value['file_storage']['tables'].append('global_table')
  with self.assertRaises(RuntimeError):validate(value)
 def test_missing_journal_rejected(self):
  value=self.good();value['file_storage']['journals'][0]['files']=[]
  with self.assertRaises(RuntimeError):validate(value)
 def test_replaced_restore_input_rejected(self):
  value=self.good();value['file_storage']['restore_inputs'][1]['files'][0]['sha256']='b'*64
  with self.assertRaises(RuntimeError):validate(value)
 def test_no_second_restore_rejected(self):
  value=self.good();value['file_storage']['restore_inputs'].pop()
  with self.assertRaises(RuntimeError):validate(value)
 def test_wrong_stage_order_rejected(self):
  value=self.good();value['file_storage']['journals'].reverse()
  with self.assertRaises(RuntimeError):validate(value)
 def test_missing_file_metadata_rejected(self):
  value=self.good();value['file_storage']['journals'][0]['files']=[{}]
  with self.assertRaises(RuntimeError):validate(value)
if __name__=='__main__':unittest.main()
