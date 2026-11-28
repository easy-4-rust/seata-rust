"""Bind actual database observations to original TC branch IDs, requests and real RM replies."""
import hashlib,json,struct
from pathlib import Path

class Reader:
 def __init__(self,data):self.data=data;self.offset=0
 def take(self,n):
  if n<0 or self.offset+n>len(self.data):raise RuntimeError('Truncated actual wire payload')
  value=self.data[self.offset:self.offset+n];self.offset+=n;return value
 def integer(self,n,signed=False):return int.from_bytes(self.take(n),'big',signed=signed)
 def string(self,n=2):return self.take(self.integer(n,True)).decode('utf-8')
 def end(self):
  if self.offset!=len(self.data):raise RuntimeError('Trailing actual wire bytes')

def decode(row):
 data=bytes.fromhex(row['frame_hex'])
 if len(data)<16 or data[:3]!=b'\xda\xda\x01' or int.from_bytes(data[3:7],'big')!=len(data) or data[10:12]!=b'\x01\x00':raise RuntimeError('Actual frame header differs')
 head=int.from_bytes(data[7:9],'big')
 if not 16<=head<=len(data):raise RuntimeError('Actual header length differs')
 kind=data[9];identifier=int.from_bytes(data[12:16],'big',signed=True);body=data[head:];code=int.from_bytes(body[:2],'big') if body else None
 if row.get('event')=='wire' and (row['id'],row['kind'],row['payload_type'],row['sha256'])!=(identifier,kind,code,hashlib.sha256(data).hexdigest()):raise RuntimeError('Wire metadata/hash differs')
 result=dict(code=code,kind=kind,id=identifier,connection=row['connection'],frame=data,sequence=row.get('sequence',0))
 if code is None:return result
 r=Reader(body[2:])
 if code==103:
  for _ in range(4):r.string()
  result['resource']=r.string(4);r.end()
 elif code in [102,104]:result['identified']=r.integer(1)==1
 elif code==11:
  result.update(xid=r.string(),branch_type=r.integer(1),resource=r.string(),lock_key=r.string(4));r.string(4);r.end()
 elif code in [3,5]:
  result.update(xid=r.string(),branch_id=r.integer(8,True),branch_type=r.integer(1),resource=r.string());r.string(4);r.end()
 elif code in [12,4,6,8,10,16]:
  result['result']=r.integer(1)
  if result['result']==0:result['message']=r.string()
  result['error']=r.integer(1)
  if code==12:result['branch_id']=r.integer(8,True)
  elif code in [4,6]:result.update(xid=r.string(),branch_id=r.integer(8,True),status=r.integer(1))
  else:result['status']=r.integer(1)
  r.end()
 return result

def marker(path,prefix):
 rows=[json.loads(line.split(prefix,1)[1]) for line in Path(path).read_bytes().splitlines() if line.startswith(prefix)]
 if len(rows)!=1:raise RuntimeError('Need one actual SDK proof marker: '+str(path))
 return rows[0]

def phase(proof,balance,strict):
 if proof['balance']!=balance or proof['branch_id']<=0 or proof['undo_status']!=0 or proof['undo_size']<=0 or len(proof['undo_sha256'])!=64:raise RuntimeError('Actual phase-one database/Undo identity differs')
 if strict:
  raw=bytes.fromhex(proof['undo_hex'])
  if len(raw)!=proof['undo_size'] or hashlib.sha256(raw).hexdigest()!=proof['undo_sha256'] or proof['process_id']<=0:raise RuntimeError('Stored Undo bytes do not match observed identity/hash')

def proxy_state(directory,prepare=False,strict=True):
 directory=Path(directory);state=json.loads((directory/'proxy-result.json').read_bytes());events=json.loads((directory/'proxy-events.json').read_bytes());errors=state['errors']
 if prepare and not strict:errors=[error for error in errors if not error.startswith('ConnectionResetError(10054,')]
 if errors or state['faults'] or state['fragmented_directions']!=['sdk-to-tc','tc-to-sdk']:raise RuntimeError('Actual AT proxy failed or synthesized results')
 if prepare and strict and sum(e['event']=='expected-phase-one-process-exit' for e in events)!=1:raise RuntimeError('Prepared SDK exit was not explicitly observed')
 return state,events

def wire_check(directory,phases,strict=True):
 directory=Path(directory);state,events=proxy_state(directory,strict=strict)
 rows=[(e,decode(e)) for e in events if e['event']=='wire'];registration={};branches={};callbacks=[]
 for event,value in rows:
  key=(value['connection'],value['id']);direction=event['direction'];code=value['code']
  if direction=='sdk-to-tc' and code==103:registration[key]=value
  elif direction=='tc-to-sdk' and code==104:
   if key not in registration or not value['identified']:raise RuntimeError('Actual RM registration not identified')
   registration[key]['accepted']=True;registration[key]['accepted_sequence']=value['sequence']
  elif direction=='sdk-to-tc' and code==11:branches[key]=value
  elif direction=='tc-to-sdk' and code==12:
   if key not in branches or value['result']!=1:raise RuntimeError('Actual branch registration rejected/unmatched')
   branches[key]['branch_id']=value['branch_id']
  elif direction=='tc-to-sdk' and code in [3,5]:callbacks.append((key,value))
 for proof,want in phases:
  phase(proof,want,strict);matches=[value for value in branches.values() if value['xid']==proof['xid'] and value.get('branch_id')==proof['branch_id']]
  if len(matches)!=1 or matches[0]['branch_type']!=0 or matches[0]['lock_key']!='ACCOUNTS:1;':raise RuntimeError('Database branch differs from actual TC-issued registration/lock')
  requested=[(key,value) for key,value in callbacks if value['xid']==proof['xid'] and value['branch_id']==proof['branch_id']]
  if len(requested)!=1 or requested[0][1]['resource']!=matches[0]['resource']:raise RuntimeError('Actual TC callback identity/resource differs')
  key,request=requested[0];expected=4 if request['code']==3 else 6;status=5 if expected==4 else 8
  replies=[value for event,value in rows if event['direction']=='sdk-to-tc' and (value['connection'],value['id'],value['code'])==(*key,expected)]
  if len(replies)!=1 or (replies[0]['xid'],replies[0]['branch_id'],replies[0]['result'],replies[0]['status'])!=(proof['xid'],proof['branch_id'],1,status):raise RuntimeError('Real RM response did not complete the original TC branch')
  if not any(reg.get('accepted') and reg['resource']==request['resource'] and reg['connection']==request['connection'] and reg['accepted_sequence']<request['sequence'] for reg in registration.values()):raise RuntimeError('Callback preceded accepted real RM registration on that connection')
 return dict(connections=state['connections'],actual_branch_registrations=len(branches),actual_callbacks=len(callbacks),actual_rm_registrations=len(registration),events=events)

def normal(directory,reconnect=False,strict=True):
 directory=Path(directory);proof=marker(directory/'client.log',b'REAL_AT_PASS ')
 if not proof['full_client_init'] or not proof['real_sql_proxy'] or proof.get('reconnect_replay',False)!=reconnect:raise RuntimeError('Actual SDK initialization/mode differs')
 result=wire_check(directory,[(proof['rollback'],900),(proof['commit'],1025)],strict)
 if strict:
  for key in ['tc_rollback_states','tc_commit_states']:
   if proof[key][-1]!=15 or any(value not in [8,9,10,11] for value in proof[key][:-1]):raise RuntimeError('Actual TC terminal reconciliation differs')
 lines=(directory/'database-final.tsv').read_text(encoding='utf-8').splitlines()
 if not lines or lines[0]!='1\t1025':raise RuntimeError('Independent SQL final business row differs')
 events=result.pop('events')
 if reconnect:
  if result['connections']!=2 or proof['defense_status']!=1 or proof['final_undo_rows']!=1:raise RuntimeError('Real reconnect/defense proof differs')
  if lines[1:]!=[proof['rollback']['xid']+'\t'+str(proof['rollback']['branch_id'])+'\t1\t2']:raise RuntimeError('Persisted duplicate-rollback fence differs')
  request=[e for e in events if e['event']=='duplicate-tc-request'];reply=[e for e in events if e['event']=='duplicate-rm-reply']
  if len(request)!=1 or len(reply)!=1:raise RuntimeError('Need one controlled duplicate instruction and real RM reply')
  duplicate=decode(request[0]);answer=decode(reply[0]);original=[decode(e) for e in events if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==5]
  if len(original)!=1 or duplicate['frame']!=original[0]['frame'] or (answer['id'],answer['code'],answer['xid'],answer['branch_id'],answer['status'])!=(duplicate['id'],6,proof['rollback']['xid'],proof['rollback']['branch_id'],8):raise RuntimeError('Duplicated instruction was changed or real RM identity/status differs')
 else:
  if lines[1:] or proof['final_undo_rows']!=0:raise RuntimeError('Normal phase-two Undo cleanup incomplete')
 result['proof']=proof;return result

def recovery(directory,strict=True):
 directory=Path(directory);prior=marker(directory/'prepare.log',b'REAL_AT_PREPARED ');proof=marker(directory/'resumed/client.log',b'REAL_AT_RECOVERY_PASS ')
 phase(prior,900,strict);phase(proof['recovery'],900,strict)
 for key in ['xid','branch_id','undo_sha256','undo_size']:
  if prior[key]!=proof['recovery'][key]:raise RuntimeError('Persisted SDK/Undo identity changed')
 if strict and (proof['original_process']!=prior['process_id'] or proof['recovery_process']!=proof['recovery']['process_id'] or proof['original_process']==proof['recovery_process']):raise RuntimeError('SDK recovery did not use a new process')
 before=(directory/'database-before-restart.tsv').read_bytes();after=(directory/'database-after-restart.tsv').read_bytes()
 if before!=after:raise RuntimeError('Business/Undo bytes changed across database restart')
 lines=before.decode('utf-8').splitlines()
 if len(lines)!=2 or lines[0]!='1\t900':raise RuntimeError('Independent phase-one SQL snapshot missing or wrong')
 fields=lines[1].split('\t')
 if len(fields)!=4 or fields[:3]!=[prior['xid'],str(prior['branch_id']),'0']:raise RuntimeError('Independent SQL does not bind original Undo identity/status')
 raw=bytes.fromhex(fields[3])
 if len(raw)!=prior['undo_size'] or hashlib.sha256(raw).hexdigest()!=prior['undo_sha256']:raise RuntimeError('Independent SQL Undo bytes differ from original SDK observation')
 if strict and raw.hex()!=prior['undo_hex']:raise RuntimeError('Independent SQL Undo content differs')
 restart=json.loads((directory/'restart.json').read_bytes())
 if restart['started_before']==restart['started_after']:raise RuntimeError('Database restart not observed')
 if (directory/'database-final.tsv').read_bytes()!=b'1\t1000\n' or proof['restored_balance']!=1000 or proof['undo_rows']!=0 or proof['second_begin_issued']:raise RuntimeError('Persistent replay did not restore original business outcome')
 if strict and proof['tc_states'][-1]!=15:raise RuntimeError('Recovered original TC transaction not Finished')
 # 原始注册发生在旧SDK连接；恢复后的回调必须对应新进程新注册的同一真实资源。
 _,first=proxy_state(directory,prepare=True,strict=strict);_,second=proxy_state(directory/'resumed',strict=strict)
 if sum(e['event']=='wire' and e['direction']=='sdk-to-tc' and e['payload_type']==1 for e in first+second)!=1:raise RuntimeError('Recovery issued a second Begin')
 original_requests=[decode(e) for e in first if e['event']=='wire' and e['direction']=='sdk-to-tc' and e['payload_type']==11];registered=[decode(e) for e in first if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==12]
 callbacks=[decode(e) for e in second if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==5]
 if len(original_requests)!=1 or len(registered)!=1 or len(callbacks)!=1 or registered[0]['result']!=1 or (original_requests[0]['connection'],original_requests[0]['id'])!=(registered[0]['connection'],registered[0]['id']) or registered[0]['branch_id']!=prior['branch_id'] or original_requests[0]['xid']!=prior['xid'] or (callbacks[0]['xid'],callbacks[0]['branch_id'])!=(prior['xid'],prior['branch_id']):raise RuntimeError('Restarted RM was not called for the original persistent branch')
 callback=callbacks[0]
 if callback['resource']!=original_requests[0]['resource'] or callback['branch_type']!=0:raise RuntimeError('Recovered TC resource/branch type changed')
 rm_requests=[decode(e) for e in second if e['event']=='wire' and e['direction']=='sdk-to-tc' and e['payload_type']==103];rm_replies=[decode(e) for e in second if e['event']=='wire' and e['direction']=='tc-to-sdk' and e['payload_type']==104]
 if not any(request['resource']==callback['resource'] and request['connection']==callback['connection'] and any(reply['identified'] and (reply['connection'],reply['id'])==(request['connection'],request['id']) and request['sequence']<reply['sequence']<callback['sequence'] for reply in rm_replies) for request in rm_requests):raise RuntimeError('Recovered real RM resource not registered before callback')
 replies=[decode(e) for e in second if e['event']=='wire' and e['direction']=='sdk-to-tc' and e['payload_type']==6]
 if len(replies)!=1 or (replies[0]['connection'],replies[0]['id'])!=(callback['connection'],callback['id']) or (replies[0]['xid'],replies[0]['branch_id'],replies[0]['status'],replies[0]['result'])!=(prior['xid'],prior['branch_id'],8,1):raise RuntimeError('Restarted real RM did not acknowledge original compensation')
 return dict(original_xid=prior['xid'],original_branch_id=prior['branch_id'],proof=proof,database_bytes_preserved=True)
