"""Independent raw MySQL binding, transaction and result verification for selected RM recovery faults."""
import copy
import hashlib
import importlib.util
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools/formal-undo-fence'))
from validate_wire import frame,lenenc,need
spec=importlib.util.spec_from_file_location('rollback_parameters',ROOT/'tools/formal-undo-rollback-loss/validate.py')
previous=importlib.util.module_from_spec(spec);spec.loader.exec_module(previous)
COMPENSATE='DELETE FROM `undo_work` WHERE `id` = ?'

TERMINAL='UPDATE seata_rs_at_branch_v1 SET terminal=1,modified_at=now(6) WHERE xid=? AND branch_id=? AND phase=2 AND terminal=0'
def rollback_statements(wire):
    remove, identity = previous.root_delete(wire, b'rollback:xid', 42)
    return {COMPENSATE:[1], remove:identity, TERMINAL:[b'rollback:xid',42]}

def ok_packet(event,in_transaction,affected):
    p=frame(event,1);need(len(p)>=7 and p[0]==0,'Truncated or non-OK server reply')
    rows,offset=lenenc(p,1);insert_id,offset=lenenc(p,offset)
    need(offset+4<=len(p) and rows==affected and insert_id==0,'Incomplete OK fields or affected rows differ')
    status=int.from_bytes(p[offset:offset+2],'little');warnings=int.from_bytes(p[offset+2:offset+4],'little')
    need(bool(status&1)==in_transaction and warnings==0,'Original transaction status or warnings differ')
    return p

def transcript(wire):
    expected=rollback_statements(wire)
    events=wire['events'];need(events and not any(e['kind']=='observer_error' for e in events),'Observer error')
    need([e['qpc_ns'] for e in events]==sorted(e['qpc_ns'] for e in events),'Event ordering')
    statements={};physical={};executes=[];last={}
    for e in events:
        k=e['kind'];c=e['connection']
        if k=='client_command':
            raw=bytes.fromhex(e['wire_hex']) if e.get('wire_hex') else None
            need(raw and hashlib.sha256(raw).hexdigest()==e['wire_sha256'] and raw[:5].hex()==e['wire_prefix_hex'] and raw[4]==e['opcode'],'Generic command bytes differ')
            last[c]=e
        elif k in ('query','prepare','execute'):
            need(c in last and last[c]['wire_hex']==e['wire_hex'],'Command event is not bound to actual raw command')
            p=frame(e,0);need(p[0]=={'query':3,'prepare':22,'execute':23}[k],'Command opcode differs')
            if k!='execute':need(p[1:].decode('utf-8')==e['sql'],'SQL text differs from original packet')
            else:
                need(len(p)>=10 and p[5]==0 and p[6:10]==b'\x01\0\0\0','Truncated or nonordinary execute command')
                statement=int.from_bytes(p[1:5],'little');need((c,statement) in statements,'Missing actual prepare binding')
                sql,count=statements[c,statement];need(e['sql']==sql and e['statement']==statement,'Executed SQL differs from actual prepare')
                if sql in expected:need(previous.parameters(e,statement,count)==expected[sql],'Original compensation or terminal identity differs')
                executes.append(e)
        elif k=='prepare_ok':
            p=frame(e,1);need(len(p)==12 and p[0]==0,'Original prepare OK differs')
            statement=int.from_bytes(p[1:5],'little');count=int.from_bytes(p[7:9],'little')
            need(any(v['kind']=='prepare' and v['connection']==c and v['sql']==e['sql'] and v['qpc_ns']<e['qpc_ns'] for v in events),'Missing original prepare command')
            statements[c,statement]=(e['sql'],count)
        elif k=='greeting':
            p=frame(e,0);need(p[0]==10,'Greeting differs');end=p.index(0,1)
            physical[c]=int.from_bytes(p[end+1:end+5],'little');need(physical[c]==e['mysql_connection_id']>0,'Physical connection differs')
        elif k in ('commit_received','commit_forwarded'):need(frame(e,0)==b'\x03COMMIT','Original COMMIT differs')
        elif k=='commit_reply':
            p=ok_packet(e,False,0);need(p.hex()==e['payload_hex'] and hashlib.sha256(p).hexdigest()==e['sha256'] and e['ok'] is True,'Original COMMIT ACK differs')
        elif k in ('commit_reply_forwarded','begin_reply_forwarded'):
            source='commit_reply' if k.startswith('commit') else 'begin_reply'
            need(any(v['kind']==source and v['connection']==c and v['wire_hex']==e['wire_hex'] and v['qpc_ns']<e['qpc_ns'] for v in events),'Forwarded ACK is not the original server ACK')
    # 原包与分类必须双向一一对应，不能依靠观察器恰好没有遗漏某个事务边界。
    for c in physical:
        commands=[e for e in events if e['connection']==c and e['kind']=='client_command']
        for i,e in enumerate(commands):
            p=frame(e,0);opcode=p[0]
            if opcode not in (3,22,23):continue
            end=commands[i+1]['qpc_ns'] if i+1<len(commands) else float('inf')
            typed=[v for v in events if v['connection']==c and v['kind']=={3:'query',22:'prepare',23:'execute'}[opcode] and e['qpc_ns']<v['qpc_ns']<end]
            need(len(typed)==1 and typed[0]['wire_hex']==e['wire_hex'],'Unclassified or multiply classified original command')
            if opcode==3 and p[1:]==b'COMMIT':
                received=[v for v in events if v['connection']==c and v['kind']=='commit_received' and typed[0]['qpc_ns']<v['qpc_ns']<end]
                need(len(received)==1 and received[0]['wire_hex']==e['wire_hex'],'Unclassified original COMMIT boundary')
            if opcode==3 and p[1:]==b'BEGIN':
                received=[v for v in events if v['connection']==c and v['kind']=='begin_received' and typed[0]['qpc_ns']<v['qpc_ns']<end]
                need(len(received)==1 and received[0]['wire_hex']==e['wire_hex'],'Unclassified original BEGIN boundary')
    return events,executes,physical

def effect_session(wire,events,executes,c,denied=False,expect_fault=False):
    expected=rollback_statements(wire)
    remove, _ = previous.root_delete(wire, b'rollback:xid', 42)
    session=[e for e in events if e['connection']==c]
    begin=[e for e in session if e['kind']=='begin_received'];need(len(begin)==1 and frame(begin[0],0)==b'\x03BEGIN','One original BEGIN required')
    ack=[e for e in session if e['kind']=='begin_reply'];need(len(ack)==1,'One original BEGIN ACK required')
    ok_packet(ack[0],True,0)
    begins=[e for e in session if e['kind']=='query' and e['sql']=='BEGIN'];forwarded=[e for e in session if e['kind']=='begin_forwarded'];delivered_begin=[e for e in session if e['kind']=='begin_reply_forwarded']
    need(len(begins)==len(forwarded)==len(delivered_begin)==1,'Exactly one raw BEGIN with original forwarded ACK required')
    need(begins[0]['qpc_ns']<begin[0]['qpc_ns']<forwarded[0]['qpc_ns']<ack[0]['qpc_ns']<delivered_begin[0]['qpc_ns'] and begins[0]['wire_hex']==begin[0]['wire_hex']==forwarded[0]['wire_hex'] and ack[0]['wire_hex']==delivered_begin[0]['wire_hex'],'BEGIN provenance differs')
    boundaries=[e['sql'] for e in session if e['kind']=='query' and e['sql'] in ('BEGIN','COMMIT','ROLLBACK')]
    # 被取消的 COMMIT future 会排队发送清理 ROLLBACK；它只能出现在唯一 COMMIT 之后，不能拆开补偿事务。
    allowed=([['BEGIN'],['BEGIN','ROLLBACK']] if denied else [['BEGIN','COMMIT'],['BEGIN','COMMIT','ROLLBACK']] if expect_fault else [['BEGIN','COMMIT']])
    need(boundaries in allowed,'Original query transaction boundaries differ')
    dml=[e for e in executes if e['connection']==c and e['sql'].startswith(('INSERT ','UPDATE ','DELETE '))]
    want=[COMPENSATE,remove] if denied else list(expected)
    need([e['sql'] for e in dml]==want,'Compensation, Undo deletion and terminal must execute in exact order in one transaction')
    for e in dml:
        need(e['qpc_ns']>ack[0]['qpc_ns'],'DML before actual BEGIN ACK')
        replies=[v for v in session if v['kind']=='dml_reply' and v['sql']==e['sql'] and v['qpc_ns']>e['qpc_ns']]
        need(len(replies)==1,'Actual DML reply required');ok_packet(replies[0],True,1)
    # Every actual execute is classified, including statements prepared before BEGIN.
    for e in session:
        if e['qpc_ns']<begin[0]['qpc_ns']:continue
        if e['kind'] in ('query','prepare','execute'):
            sql=e['sql'];need(';' not in sql and '/*' not in sql and '--' not in sql,'Unexpected SQL boundary')
            read_or_dml=sql in expected or sql=='SHOW GRANTS' or sql.startswith('SELECT ')
            need(read_or_dml or (e['kind']=='query' and sql in ('BEGIN','COMMIT','ROLLBACK')),'Prepared or unexpected SQL transaction boundary')
        if e['kind']=='client_command':need(e['opcode'] in (1,3,22,23,25),'Unexpected protocol command in control transaction')
    if denied:
        errors=[e for e in session if e['kind']=='prepare_error' and e['sql']==TERMINAL]
        need(len(errors)==1,'Actual denied terminal prepare required');p=frame(errors[0],1);need(p[0]==255 and int.from_bytes(p[1:3],'little')==1142,'Actual terminal UPDATE privilege error required')
        last_reply=next(e for e in session if e['kind']=='dml_reply' and e['sql']==remove)
        need(errors[0]['qpc_ns']>last_reply['qpc_ns'],'Terminal denial must follow successful Undo deletion')
        need(not any(e['kind']=='commit_received' for e in session),'Failed compensation transaction must not commit')
    else:
        commit=[e for e in session if e['kind']=='commit_received'];need(len(commit)==1 and commit[0]['qpc_ns']>next(e for e in session if e['kind']=='dml_reply' and e['sql']==TERMINAL)['qpc_ns'],'COMMIT must follow successful terminal update')
        need(not any(e['kind']=='query' and e['sql']=='ROLLBACK' for e in session if e['qpc_ns']<commit[0]['qpc_ns']),'Intermediate ROLLBACK splits the transaction')
        need(commit[0]['fault'] is expect_fault,'Fault label differs from the independently selected session role')
        if not expect_fault:
            forward=[e for e in session if e['kind']=='commit_forwarded'];reply=[e for e in session if e['kind']=='commit_reply'];delivered=[e for e in session if e['kind']=='commit_reply_forwarded']
            need(len(forward)==len(reply)==len(delivered)==1,'Nonfault effect COMMIT requires actual forwarded ACK')
            need(commit[0]['qpc_ns']<forward[0]['qpc_ns']<reply[0]['qpc_ns']<delivered[0]['qpc_ns'] and reply[0]['wire_hex']==delivered[0]['wire_hex'],'Nonfault COMMIT ACK provenance differs');ok_packet(reply[0],False,0)
    return session

def validate(wire,observation):
    events,executes,physical=transcript(wire);mode=wire['mode'];need(mode in ('drop_ok','block_commit','delay_ok') and wire['claimed'] is True,'Missing actual fault')
    candidates=[e['connection'] for e in executes if e['sql']==TERMINAL]
    need(len(candidates)==(2 if mode=='block_commit' else 1) and len(set(candidates))==len(candidates),'Actual compensation attempts differ')
    for i,c in enumerate(candidates):effect_session(wire,events,executes,c,expect_fault=i==0)
    fault=[e for e in events if e['kind']=='commit_received' and e['fault']];need(len(fault)==1 and fault[0]['connection']==candidates[0],'Fault was not on the effect transaction')
    c=candidates[0];session=[e for e in events if e['connection']==c]
    forwarded=[e for e in session if e['kind']=='commit_forwarded'];replies=[e for e in session if e['kind']=='commit_reply'];delivered=[e for e in session if e['kind']=='commit_reply_forwarded']
    need(not delivered,'First effect COMMIT must lack delivered confirmation')
    need(len(forwarded)==len(replies)==(0 if mode=='block_commit' else 1),'Fault forwarding/actual ACK differs')
    if mode=='delay_ok':
        held=next(e for e in session if e['kind']=='commit_reply_held');late=next(e for e in session if e['kind'] in ('late_reply_sent','late_reply_peer_closed'))
        need(late['qpc_ns']-held['qpc_ns']>=2_100_000_000,'Reply was not actually delayed past deadline')
    if mode=='block_commit':
        recovery=[e for e in events if e['connection']==candidates[1]]
        need(len([e for e in recovery if e['kind']=='commit_reply_forwarded'])==1,'Recovery COMMIT actual confirmation required')
    need(observation['mode']==mode and observation['first']=='Unknown' and observation['recovered']=='Applied' and observation['xid']=='rollback:xid' and observation['branch_id']==42,'Original observed identity or result differs')
    need(observation['before']==([1,1,[1,2,0]] if mode=='block_commit' else [0,0,[1,2,1]]) and observation['after']==[0,0,[1,2,1]],'Independent durable business/Undo/terminal facts differ')
    return dict(status='RAW_RECOVERY_ATOMICITY_VERIFIED',mode=mode,physical_effect_connections=[physical[c] for c in candidates],actual_compensations=len(candidates),actual_terminal_updates=len(candidates),automatic_business_replays=0)

def validate_denial(wire,observation):
    need(wire['mode']=='observe' and wire['claimed'] is False,'Denial role must not inject a commit fault')
    need(all(e['fault'] is False for e in wire['events'] if e['kind']=='commit_received'),'Observe role cannot relabel missing ACK as a fault')
    events,executes,physical=transcript(wire)
    denied=[e for e in events if e['kind']=='prepare_error' and e['sql']==TERMINAL];need(len(denied)==1,'One actual terminal denial required')
    effect_session(wire,events,executes,denied[0]['connection'],True)
    recovered=[e for e in executes if e['sql']==TERMINAL];need(len(recovered)==1,'One repaired recovery terminal required');effect_session(wire,events,executes,recovered[0]['connection'])
    need(observation['before']==[1,1,[1,2,0]] and observation['after']==[0,0,[1,2,1]],'Independent aborted/recovered DB facts differ')
    return dict(status='ACTUAL_DML_SUCCEEDED_BEFORE_TERMINAL_DENIAL_AND_ROLLED_BACK',denied_physical_connection=physical[denied[0]['connection']],recovered_physical_connection=physical[recovered[0]['connection']])

def validate_concurrency(wire,observation):
    remove, _ = previous.root_delete(wire, b'rollback:xid', 42)
    need(wire['mode']=='observe' and wire['claimed'] is False,'Concurrency role must not inject a commit fault')
    need(all(e['fault'] is False for e in wire['events'] if e['kind']=='commit_received'),'Observe role cannot relabel missing ACK as a fault')
    events,executes,physical=transcript(wire)
    terminal=[e for e in executes if e['sql']==TERMINAL];need(len(terminal)==1,'Concurrent attempts must publish one physical terminal')
    effect_session(wire,events,executes,terminal[0]['connection'])
    need(sum(e['sql']==COMPENSATE for e in executes)==sum(e['sql']==remove for e in executes)==1,'Concurrent attempts repeated compensation or Undo deletion')
    need(observation['actual_competing_lock_waits']>=2 and observation['applied']>=1 and observation['terminal']==[1,2,1] and observation['undo']==0 and observation['later_business']=='late-owner','Actual competing attempts or final DB facts differ')
    return dict(status='CONTENDING_INFLIGHT_ATTEMPTS_ONE_ATOMIC_EFFECT',actual_competing_lock_waits=observation['actual_competing_lock_waits'],physical_effect_connection=physical[terminal[0]['connection']],compensations=1,terminal_updates=1)

def negative_controls(wire,observation):
    remove, _ = previous.root_delete(wire, b'rollback:xid', 42)
    result=[]
    for mode in ('terminal_wrong_xid','compensation_wrong_pk','missing_dml_reply','wrong_commit_connection','forwarded_ERR','intermediate_ROLLBACK','raw_unclassified_ROLLBACK','prepared_COMMIT','preprepared_COMMIT','truncated_commit_ACK','missing_commit_ACK','intermediate_BEGIN'):
        bad=copy.deepcopy(wire)
        if mode in ('terminal_wrong_xid','compensation_wrong_pk'):
            e=next(e for e in bad['events'] if e['kind']=='execute' and e['sql']==(TERMINAL if mode=='terminal_wrong_xid' else COMPENSATE));old=e['wire_hex'];raw=bytearray.fromhex(old)
            if mode=='terminal_wrong_xid':raw=raw.replace(b'rollback:xid',b'rollback:bad')
            else:raw[-8:]=(2).to_bytes(8,'little',signed=True)
            e['wire_hex']=raw.hex();linked=next(v for v in bad['events'] if v['kind']=='client_command' and v['connection']==e['connection'] and v['wire_hex']==old);linked.update(wire_hex=raw.hex(),wire_sha256=hashlib.sha256(raw).hexdigest(),wire_prefix_hex=raw[:5].hex())
        elif mode=='missing_dml_reply':bad['events'].remove(next(e for e in bad['events'] if e['kind']=='dml_reply' and e['sql']==remove))
        elif mode=='wrong_commit_connection':next(e for e in bad['events'] if e['kind']=='commit_received' and e['fault'])['connection']=99999
        elif mode=='forwarded_ERR':
            e=next(e for e in bad['events'] if e['kind']=='begin_reply_forwarded');raw=bytearray.fromhex(e['wire_hex']);raw[4]=255;e['wire_hex']=raw.hex()
        elif mode in ('intermediate_ROLLBACK','raw_unclassified_ROLLBACK','intermediate_BEGIN'):
            e=next(e for e in bad['events'] if e['kind']=='execute' and e['sql']==remove);sql='BEGIN' if mode=='intermediate_BEGIN' else 'ROLLBACK';p=b'\x03'+sql.encode();raw=len(p).to_bytes(3,'little')+b'\0'+p
            bad['events'].append(dict(kind='client_command',connection=e['connection'],qpc_ns=e['qpc_ns']+1,opcode=3,wire_hex=raw.hex(),wire_prefix_hex=raw[:5].hex(),wire_sha256=hashlib.sha256(raw).hexdigest()))
            if mode!='raw_unclassified_ROLLBACK':bad['events'].append(dict(kind='query',connection=e['connection'],qpc_ns=e['qpc_ns']+2,sql=sql,wire_hex=raw.hex()))
        elif mode in ('prepared_COMMIT','preprepared_COMMIT'):
            first=next(e for e in bad['events'] if e['kind']=='execute' and e['sql']==COMPENSATE);c=first['connection'];t=first['qpc_ns']+1
            prepare_t=t if mode=='prepared_COMMIT' else next(e for e in bad['events'] if e['kind']=='begin_received' and e['connection']==c)['qpc_ns']-1000
            statement=12345678
            def add(payload,kind,stamp,**fields):
                raw=len(payload).to_bytes(3,'little')+b'\0'+payload
                bad['events'].extend([dict(kind='client_command',connection=c,qpc_ns=stamp,opcode=payload[0],wire_hex=raw.hex(),wire_prefix_hex=raw[:5].hex(),wire_sha256=hashlib.sha256(raw).hexdigest()),dict(kind=kind,connection=c,qpc_ns=stamp+1,wire_hex=raw.hex(),**fields)])
            add(b'\x16COMMIT','prepare',prepare_t,sql='COMMIT')
            payload=b'\0'+statement.to_bytes(4,'little')+b'\0'*7;raw=len(payload).to_bytes(3,'little')+b'\1'+payload
            bad['events'].append(dict(kind='prepare_ok',connection=c,qpc_ns=prepare_t+2,wire_hex=raw.hex(),sql='COMMIT'))
            add(b'\x17'+statement.to_bytes(4,'little')+b'\0\x01\0\0\0','execute',t+3,sql='COMMIT',statement=statement)
        elif mode=='truncated_commit_ACK':
            e=next(e for e in bad['events'] if e['kind']=='commit_reply');p=b'\0\0\0';raw=len(p).to_bytes(3,'little')+b'\1'+p;e.update(wire_hex=raw.hex(),payload_hex=p.hex(),sha256=hashlib.sha256(p).hexdigest())
        else:bad['events']=[e for e in bad['events'] if e['kind'] not in ('commit_reply','commit_reply_forwarded')]
        bad['events'].sort(key=lambda e:e['qpc_ns'])
        try:validate(bad,observation)
        except (ValueError,KeyError,StopIteration):result.append(dict(mutation=mode,status='REJECTED'))
        else:raise ValueError('Wire negative accepted: '+mode)
    return result

def observer_negative_controls(wire,observation,validator):
    controls=[]
    for name in ('missing_all_commit_ACKs','truncated_commit_ACK','unclassified_ROLLBACK','intermediate_BEGIN','fake_fault_without_ACK'):
        bad=copy.deepcopy(wire)
        if name in ('missing_all_commit_ACKs','fake_fault_without_ACK'):
            bad['events']=[e for e in bad['events'] if e['kind'] not in ('commit_reply','commit_reply_forwarded')]
            if name=='fake_fault_without_ACK':
                for e in bad['events']:
                    if e['kind']=='commit_received':e['fault']=True
        elif name=='truncated_commit_ACK':
            e=next(e for e in bad['events'] if e['kind']=='commit_reply');p=b'\0\0\0';raw=len(p).to_bytes(3,'little')+b'\1'+p;e.update(wire_hex=raw.hex(),payload_hex=p.hex(),sha256=hashlib.sha256(p).hexdigest())
        else:
            e=next(e for e in bad['events'] if e['kind']=='execute' and e['sql']==COMPENSATE);sql='BEGIN' if name=='intermediate_BEGIN' else 'ROLLBACK';p=b'\x03'+sql.encode();raw=len(p).to_bytes(3,'little')+b'\0'+p
            bad['events'].append(dict(kind='client_command',connection=e['connection'],qpc_ns=e['qpc_ns']+1,opcode=3,wire_hex=raw.hex(),wire_prefix_hex=raw[:5].hex(),wire_sha256=hashlib.sha256(raw).hexdigest()));bad['events'].sort(key=lambda e:e['qpc_ns'])
            if name=='intermediate_BEGIN':bad['events'].append(dict(kind='query',connection=e['connection'],qpc_ns=e['qpc_ns']+2,sql=sql,wire_hex=raw.hex()));bad['events'].sort(key=lambda e:e['qpc_ns'])
        try:validator(bad,observation)
        except (ValueError,KeyError,StopIteration):controls.append(dict(mutation=name,status='REJECTED'))
        else:raise ValueError('Observer negative accepted: '+name)
    return controls
