"""Unit examples verify defect classification, not synthetic recovery evidence."""
import copy,unittest
from evidence import verify_namespace
class NamespaceGuards(unittest.TestCase):
 def good(self):return dict(java_xid='owned:28091:1',go_xid='owned:28091:2',java_branch=11,go_branch=22,java_resource='jdbc:mysql://127.0.0.1:28093/owned',go_resource='fixture-user:fixture-password@tcp(127.0.0.1:28093)/owned',java_lock='accounts:1',go_lock='ACCOUNTS:1;',java_balance=900,both_balance=800,final_balance=800,java_undo_branches=[11],both_undo_branches=[11,22],final_undo_rows=0,before={ 'java-java':False,'go-java':True,'java-go':True,'go-go':True},after={'java-java':False,'go-java':False,'java-go':True,'go-go':False},final={key:True for key in ['java-java','go-java','java-go','go-go']},tm_counts={'java':[1,1,0],'go':[1,1,0]},original_accepted_branches=True)
 def test_control(self):verify_namespace(self.good())
 def bad(self,fn):
  value=self.good();fn(value)
  with self.assertRaises(RuntimeError):verify_namespace(value)
 def test_same_xid_cannot_prove_shared_mutex_failure(self):self.bad(lambda v:v.update(go_xid=v['java_xid']))
 def test_same_resource_cannot_prove_resource_namespace_divergence(self):self.bad(lambda v:v.update(go_resource=v['java_resource']))
 def test_java_lock_was_never_held(self):self.bad(lambda v:v['before'].update({'java-java':True}))
 def test_other_resource_not_independently_free(self):self.bad(lambda v:v['before'].update({'go-java':False}))
 def test_second_at_did_not_commit(self):self.bad(lambda v:v.update(both_balance=900))
 def test_second_undo_not_persisted(self):self.bad(lambda v:v.update(both_undo_branches=[11]))
 def test_final_undo_not_clean(self):self.bad(lambda v:v.update(final_undo_rows=1))
 def test_go_lock_not_held_after_phase_one(self):self.bad(lambda v:v['after'].update({'go-go':True}))
 def test_final_lock_not_released(self):self.bad(lambda v:v['final'].update({'go-go':False}))
 def test_unaccepted_branch_not_real_business(self):self.bad(lambda v:v.update(original_accepted_branches=False))
 def test_second_begin_hides_retry(self):self.bad(lambda v:v['tm_counts'].update(go=[2,1,0]))
if __name__=='__main__':unittest.main()
