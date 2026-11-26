"""Unit-only examples of report binding; never substitute for actual cluster recovery."""
import copy,hashlib,json,unittest
from context_evidence import verify_updated_context
class UpdatedContextGuards(unittest.TestCase):
 def good(self):
  phase=dict(xid='owned:28291:7',tcc_branch_id=8);token=hashlib.sha256(b'owned:28291:7/8/10').hexdigest();registered=json.dumps(dict(actionContext={'actionName':'owned'}));data=json.loads(registered);data['actionContext'].update({'prepared-units':'10','prepared-branch':'8','prepared-token':token});reported=json.dumps(data)
  return dict(phase=phase,registered=registered,reported=reported,replicas=[reported]*12,callbacks=[reported]*3,prepared=[['owned:28291:7','8','10',token]]*5,accepted=[token,token])
 def check(self,value):return verify_updated_context(**value)
 def test_control(self):self.check(self.good())
 def bad(self,fn):
  value=self.good();fn(value)
  with self.assertRaises(RuntimeError):self.check(value)
 def test_unchanged_prepare_context_rejected(self):self.bad(lambda v:v.update(registered=v['reported']))
 def test_missing_report_rejected(self):self.bad(lambda v:v.update(reported=None))
 def test_old_context_on_all_replicas_rejected(self):self.bad(lambda v:v.update(replicas=[v['registered']]*12))
 def test_one_replica_loses_update(self):self.bad(lambda v:v['replicas'].__setitem__(0,v['registered']))
 def test_elected_callback_loses_update(self):self.bad(lambda v:v['callbacks'].__setitem__(1,v['registered']))
 def test_replayed_callback_loses_update(self):self.bad(lambda v:v['callbacks'].__setitem__(2,v['registered']))
 def test_missing_replica_observation(self):self.bad(lambda v:v.update(replicas=v['replicas'][:-1]))
 def test_missing_callback_observation(self):self.bad(lambda v:v.update(callbacks=v['callbacks'][:-1]))
 def test_durable_token_mismatch(self):self.bad(lambda v:v['prepared'].__setitem__(0,[v['phase']['xid'],'8','10','0'*64]))
 def test_missing_final_sql_row(self):self.bad(lambda v:v.update(prepared=v['prepared'][:-1]))
 def test_business_accepted_wrong_token(self):self.bad(lambda v:v['accepted'].__setitem__(0,'0'*64))
 def test_missing_replay_acceptance(self):self.bad(lambda v:v.update(accepted=v['accepted'][:1]))
 def test_report_missing_actual_generated_key(self):
  def action(v):
   data=json.loads(v['reported']);del data['actionContext']['prepared-token'];v['reported']=json.dumps(data);v['replicas']=[v['reported']]*12;v['callbacks']=[v['reported']]*3
  self.bad(action)
 def test_report_wrong_prepared_branch(self):
  def action(v):
   data=json.loads(v['reported']);data['actionContext']['prepared-branch']='9';v['reported']=json.dumps(data);v['replicas']=[v['reported']]*12;v['callbacks']=[v['reported']]*3
  self.bad(action)
if __name__=='__main__':unittest.main()
