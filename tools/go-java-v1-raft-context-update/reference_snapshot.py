"""Bind real mixed-mode branches, persisted effects, operator boundaries and lost frames."""
import hashlib,importlib.util,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('commit_actual_wire',ROOT/'tools/go-java-v1-real-at/verify.py');wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)
spec=importlib.util.spec_from_file_location('file_commit_journal_helpers',ROOT/'tools/go-java-v1-file-at-timeout/evidence.py');file_helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(file_helpers)
ACTION='owned-real-commit-action'

def snapshot(path,phase):
 path=Path(path);name=path.stem;rows=[line.split('\t') for line in path.read_text(encoding='utf-8').splitlines()]
 names=['business','undo','commit_account','reservation'];groups={key:[row[1:] for row in rows if row[0]==key] for key in names};widths=dict(business=2,undo=4,commit_account=3,reservation=4)
 if not rows or len(rows)!=sum(map(len,groups.values())) or any(len(row)!=widths[key] for key,values in groups.items() for row in values) or groups['business']!=[['1','900']] or len(groups['undo'])>1 or len(groups['commit_account'])!=1 or len(groups['reservation'])!=1:raise RuntimeError('Independent mixed business snapshot missing or malformed')
 undo=groups['undo'][0] if groups['undo'] else None
 if undo and (undo[0]!=phase['xid'] or int(undo[1])!=phase['branch_id']):raise RuntimeError('Original AT Undo identity differs')
 account=groups['commit_account'][0];reservation=groups['reservation'][0]
 if account[0]!='1' or reservation[0]!=phase['xid'] or int(reservation[1])!=phase['tcc_branch_id']:raise RuntimeError('Original persistent TCC business identity differs')
 body=json.loads((path.parent/('console-'+name.replace('_','-')+'.json')).read_bytes());data=body.get('data',[]);final=name=='final';before=name in ['timeout','restart','rejected']
 if str(body.get('code'))!='200' or body.get('total')!=(0 if final else 1) or len(data)!=(0 if final else 1):raise RuntimeError('Actual RAFT reference console session count differs')
 branches=[]
 if not final:
  session=data[0];branches=session.get('branchSessionVOs') or [];expected={phase['branch_id'],phase['tcc_branch_id']} if before else {phase['tcc_branch_id']}
  if session.get('xid')!=phase['xid'] or session.get('status')!=16 or {int(branch['branchId']) for branch in branches}!=expected or len(branches)!=len(expected):raise RuntimeError('Actual RAFT reference console original16 branches differ')
  for branch in branches:
   at=int(branch['branchId'])==phase['branch_id']
   if branch.get('xid')!=phase['xid'] or branch.get('branchType')!=('AT' if at else 'TCC') or branch.get('resourceId')!=(phase['resource_id'] if at else ACTION) or branch.get('status') is None:raise RuntimeError('Original RAFT reference branch resource/type/status differs')
 native=json.loads((path.parent/('native-'+name+'.json')).read_bytes());frames=native['frames']
 if frames!=json.loads((path.parent/('native-'+name+'-frames.json')).read_bytes()) or len(frames)!=6 or any(hashlib.sha256(bytes.fromhex(frame['frame_hex'])).hexdigest()!=frame['sha256'] for frame in frames):raise RuntimeError('Actual native trace/hash differs')
 requests=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='observer-to-tc'];responses=[bytes.fromhex(f['frame_hex']) for f in frames if f['direction']=='tc-to-observer']
 if len(requests)!=3 or len(responses)!=3 or [int.from_bytes(frame[16:18],'big') for frame in requests]!=[101,15,21]:raise RuntimeError('Native observer must send only read-only TM queries')
 stream=file_helpers.ReplayStream(b''.join(responses));observed=file_helpers.observer.query(stream,phase['xid'],phase['resource_id'],'ACCOUNTS:1;',15 if final else 16,True)
 if stream.data or bytes(stream.sent)!=b''.join(requests) or observed!=native:raise RuntimeError('Actual native frames do not reproduce RAFT reference commit query')
 return dict(xid=phase['xid'],balance=900,status=None if final else 16,at_branch=phase['branch_id'] if before else None,tcc_branch=None if final else phase['tcc_branch_id'],locks=0,undo_hex=undo[3] if undo else None,undo_status=int(undo[2]) if undo else None,reserved=int(account[1]),confirmed=int(account[2]),effect_count=int(reservation[3]),tcc_phase=reservation[2],tcc_xid=reservation[0],tcc_effect_branch=int(reservation[1]),stored=groups,tc_branches=branches)
