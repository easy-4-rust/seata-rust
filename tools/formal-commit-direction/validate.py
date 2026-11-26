"""Independent original MySQL commands, commit ACKs and actual Pending/Completed journals."""
import copy
import hashlib
import importlib.util
import struct
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
previous=module('commit_direction_original_frames',ROOT/'tools/formal-first-phase-preparation/validate.py')
base=previous.base
reader=module('commit_direction_original_journal',ROOT/'tools/formal-rm-java/validate.py')
need,frame=base.need,base.frame
ROOT_DELETE='DELETE FROM undo_log WHERE xid=? AND branch_id=?'
AUTHORIZE='UPDATE seata_rs_at_branch_v1 SET phase=1,commit_authorized=1,modified_at=now(6) WHERE xid=? AND branch_id=? AND phase IN (0,1) AND terminal=0 AND commit_authorized=0 AND commit_terminal=0'
TERMINAL='UPDATE seata_rs_at_branch_v1 SET commit_terminal=1,modified_at=now(6) WHERE xid=? AND branch_id=? AND phase=1 AND terminal=0 AND commit_authorized=1 AND commit_terminal=0'
ENROLL='INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,application_data,lock_key,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(3,?,?,?,?,?,1,?,0,0,now(6),now(6))'
FIXTURE_BUSINESS="INSERT INTO undo_work VALUES(1,'committed-fault-fixture')"
FIXTURE_LOG="INSERT INTO undo_log VALUES(?,'rollback:xid',?,?,0,now(6),now(6))"
# 固定、独立审查的当前夹具语句集；绝不从待校验报文自动生成许可。
APPROVED=json.loads((Path(__file__).parent/'approved-sql.json').read_bytes())
READS=set(APPROVED['reads']);SETUP=set(APPROVED['setup'])
CONNECTION_SETUP={s for s in SETUP if s.startswith('SET ') and not s.startswith('SET GLOBAL ')}

def one(rows,kind,sql=None):
    selected=[e for e in rows if e['kind']==kind and (sql is None or e.get('sql')==sql)]
    need(len(selected)==1,'Unique original event required: '+kind+' '+str(sql));return selected[0]

def values(events,event):
    statement=event['statement'];connection=event['connection']
    replies=[e for e in events if e['connection']==connection and e['kind']=='prepare_ok' and int.from_bytes(frame(e,1)[1:5],'little')==statement and e['qpc_ns']<event['qpc_ns']]
    need(len(replies)==1,'Unique prepared argument metadata required')
    if int.from_bytes(frame(replies[0],1)[7:9],'little')==0:
        need(len(frame(event,0))==10,'Unexpected parameters on zero-argument statement');return []
    return previous.bound_values(events,event)

def delivered(rows,event,in_transaction=True):
    later=[e['qpc_ns'] for e in rows if e['connection']==event['connection'] and e['kind']=='execute' and e['sql']==event['sql'] and e['qpc_ns']>event['qpc_ns']]
    end=min(later) if later else float('inf')
    window=[e for e in rows if e['connection']==event['connection'] and event['qpc_ns']<e['qpc_ns']<end]
    reply=one(window,'dml_reply',event['sql']);copy_event=one(window,'dml_reply_forwarded',event['sql'])
    need(event['qpc_ns']<reply['qpc_ns']<copy_event['qpc_ns'] and reply['wire_hex']==copy_event['wire_hex'],'DML original ACK provenance differs')
    base.ok_packet(reply,in_transaction,1)
    return copy_event

def session(events,executes,connection,expected,aborted=False,unknown_commit=False):
    rows=[e for e in events if e['connection']==connection]
    begin=one(rows,'begin_received');forward=one(rows,'begin_forwarded');ack=one(rows,'begin_reply');copy_event=one(rows,'begin_reply_forwarded')
    need(frame(begin,0)==frame(forward,0)==b'\x03BEGIN' and begin['qpc_ns']<forward['qpc_ns']<ack['qpc_ns']<copy_event['qpc_ns'] and ack['wire_hex']==copy_event['wire_hex'],'Original BEGIN/ACK provenance differs')
    base.ok_packet(ack,True,0)
    mutations=[e for e in executes if e['connection']==connection and e['sql'].startswith(('INSERT ','DELETE ','UPDATE '))]
    need([e['sql'] for e in mutations]==expected and all(copy_event['qpc_ns']<e['qpc_ns'] for e in mutations),'Unexpected or premature transaction mutation')
    boundaries=[e for e in rows if e['kind']=='query' and e['sql'] in ['BEGIN','COMMIT','ROLLBACK']]
    ending=[e['sql'] for e in boundaries]
    permitted=[['BEGIN'],['BEGIN','ROLLBACK']] if aborted else ([['BEGIN','COMMIT'],['BEGIN','COMMIT','ROLLBACK']] if unknown_commit else [['BEGIN','COMMIT']])
    need(ending in permitted,'Unexpected original transaction boundary sequence')
    # 超时后驱动可能在已尝试COMMIT之后发送清理ROLLBACK；它不允许包围任何后续资源语句。
    end=boundaries[1]['qpc_ns'] if len(boundaries)>1 else float('inf')
    need(all(e['qpc_ns']<end for e in mutations),'Mutation crossed the original end boundary')
    for e in rows:
        if e['kind'] in ['prepare','execute']:
            allowed=e['sql'] in READS or e['sql'] in expected or (e['sql'] in CONNECTION_SETUP and e['qpc_ns']<begin['qpc_ns'])
            need(allowed,'Unexpected prepared SQL in selected physical session')
            if e['kind']=='execute':
                need(e['qpc_ns']<end and (e['qpc_ns']>copy_event['qpc_ns'] or e['sql'] in READS or e['sql'] in CONNECTION_SETUP),'Execute crossed original BEGIN/COMMIT boundary')
        if e['kind']=='query' and e['qpc_ns']>begin['qpc_ns']:
            need(e['sql'] in ['COMMIT','ROLLBACK'] or (e['sql'] in READS and e['qpc_ns']<end),'Hidden original query inside transaction')
        if e['kind']=='client_command' and e['qpc_ns']>begin['qpc_ns']:
            need(e['opcode'] in [1,3,22,23,25],'Hidden reset/change-user command inside transaction')
    return rows,mutations,begin

def whole_packet(events,executes,observation,attempts):
    allowed={ENROLL,AUTHORIZE,ROOT_DELETE,TERMINAL,FIXTURE_BUSINESS,FIXTURE_LOG}
    for e in events:
        if e['kind'] in ['prepare','execute']:need(e['sql'] in READS or e['sql'] in SETUP or e['sql'] in allowed,'Unknown prepared statement outside selected effect sessions')
        if e['kind']=='query':need(e['sql'] in READS or e['sql'] in SETUP or e['sql'] in ['BEGIN','COMMIT','ROLLBACK'],'Unknown raw query or hidden raw write')
    expected={ENROLL:1,AUTHORIZE:1,ROOT_DELETE:attempts,TERMINAL:attempts,FIXTURE_BUSINESS:1,FIXTURE_LOG:2}
    for sql,count in expected.items():need(sum(e['sql']==sql for e in executes)==count,'Whole-packet mutation count differs: '+sql)
    business=one(executes,'execute',FIXTURE_BUSINESS);logs=[e for e in executes if e['sql']==FIXTURE_LOG]
    admin=business['connection'];auth=one(executes,'execute',AUTHORIZE);enroll=one(executes,'execute',ENROLL)
    auth_begin=one([e for e in events if e['connection']==auth['connection']],'begin_received')
    need(values(events,business)==[] and business['qpc_ns']<min(e['qpc_ns'] for e in logs) and all(e['connection']==admin and e['qpc_ns']<auth_begin['qpc_ns'] for e in logs),'Fixture business/log timing or physical source differs')
    need([values(events,e) for e in logs]==[[42,b'serializer=opaque&compressorType=GZIP',b'opaque-body'],[99,b'serializer=sentinel',b'opaque-body']],'Fixture original/sentinel identities differ')
    delivered(events,business,False)
    for e in logs:delivered(events,e,False)
    for sql in SETUP-CONNECTION_SETUP:
        executed=[e for e in events if e['kind'] in ['query','execute'] and e['sql']==sql]
        need(len(executed)==1 and executed[0]['connection']==admin and executed[0]['qpc_ns']<enroll['qpc_ns'],'Fixture DDL/global setup repeated or escaped initialization')
        if executed[0]['kind']=='execute':need(values(events,executed[0])==[],'Unexpected setup parameters')
    for c in {e['connection'] for e in events}:
        rows=[e for e in events if e['connection']==c]
        barriers=[e['qpc_ns'] for e in rows if (e['kind']=='query' and e['sql']=='BEGIN') or (e['kind']=='execute' and e['sql'] in allowed)]
        for sql in CONNECTION_SETUP:
            actual=[e for e in rows if e['kind'] in ['query','execute'] and e['sql']==sql]
            need(len(actual)<=1 and all(e['qpc_ns']<min(barriers) for e in actual) if barriers else len(actual)<=1,'Connection setup repeated or crossed an active boundary')
            for e in actual:
                if e['kind']=='execute':need(values(events,e)==[],'Unexpected connection setup arguments')
    enrollment_rows,_,_=session(events,executes,enroll['connection'],[ENROLL])
    delivery=delivered(enrollment_rows,enroll);previous.verify_control_commit(enrollment_rows,[delivery],False,observation['mode'])
    need(one(enrollment_rows,'commit_reply_forwarded')['qpc_ns']<business['qpc_ns'],'Fixture began before original enrollment confirmation')

def injected_case(wire,kind):
    """Pure-memory, fully bound protocol counterexamples for the two reviewed scope holes."""
    events=wire['events'];inserted=[]
    def packet(payload,sequence):return len(payload).to_bytes(3,'little')+bytes([sequence])+payload
    def add(connection,tag,at,**fields):inserted.append(dict(connection=connection,kind=tag,qpc_ns=at,**fields))
    def client(connection,opcode,wire,at):add(connection,'client_command',at,opcode=opcode,wire_hex=wire.hex(),wire_sha256=hashlib.sha256(wire).hexdigest(),wire_prefix_hex=wire[:5].hex())
    def prepared(connection,sql,statement,at):
        raw=packet(b'\x16'+sql.encode(),0);client(connection,22,raw,at);add(connection,'prepare',at+1,sql=sql,wire_hex=raw.hex())
        ok=packet(struct.pack('<BIHHBH',0,statement,0,0,0,0),1);add(connection,'prepare_ok',at+2,sql=sql,wire_hex=ok.hex())
    def execute(connection,sql,statement,at,dml=False):
        raw=packet(b'\x17'+struct.pack('<IBI',statement,0,1),0);client(connection,23,raw,at);add(connection,'execute',at+1,sql=sql,statement=statement,wire_hex=raw.hex())
        if dml:
            ok=packet(b'\x00\x00\x00\x03\x00\x00\x00',1)
            add(connection,'dml_reply',at+2,sql=sql,wire_hex=ok.hex());add(connection,'dml_reply_forwarded',at+3,sql=sql,wire_hex=ok.hex())
    effect=next(e for e in events if e['kind']=='execute' and e['sql']==TERMINAL);c=effect['connection']
    deleted=one([e for e in events if e['connection']==c],'dml_reply_forwarded',ROOT_DELETE)
    next_id=max(e.get('statement',0) for e in events if e['connection']==c)+1000
    if kind in ['prepared-boundaries-inside','prepared-before-BEGIN-executed-inside']:
        start=deleted['qpc_ns']+1
        for index,sql in enumerate(['COMMIT','BEGIN']):
            statement=next_id+index
            if kind=='prepared-before-BEGIN-executed-inside':
                begin=one([e for e in events if e['connection']==c],'begin_received')
                generic=next(e for e in events if e['connection']==c and e['kind']=='client_command' and e['wire_hex']==begin['wire_hex'])
                at=generic['qpc_ns']-100+index*4
            else:at=start+index*8
            prepared(c,sql,statement,at);execute(c,sql,statement,start+index*8+4)
    elif kind=='mid-transaction-raw-ROLLBACK':
        at=deleted['qpc_ns']+1;raw=packet(b'\x03ROLLBACK',0);client(c,3,raw,at)
        for offset,tag in enumerate(['query','rollback_received','rollback_forwarded'],1):
            add(c,tag,at+offset,wire_hex=raw.hex(),**({'sql':'ROLLBACK'} if tag=='query' else {}))
        ok=packet(b'\x00\x00\x00\x02\x00\x00\x00',1)
        add(c,'rollback_reply',at+4,wire_hex=ok.hex());add(c,'rollback_reply_forwarded',at+5,wire_hex=ok.hex())
    elif kind=='extra-enrollment-business-UPDATE':
        enroll=next(e for e in events if e['kind']=='execute' and e['sql']==ENROLL);c=enroll['connection']
        commit=one([e for e in events if e['connection']==c],'commit_received')
        generic=next(e for e in events if e['connection']==c and e['kind']=='client_command' and e['wire_hex']==commit['wire_hex'])
        at=generic['qpc_ns']-100;statement=max(e.get('statement',0) for e in events if e['connection']==c)+1000
        sql="UPDATE undo_work SET v='committed-fault-fixture' WHERE id=1"
        prepared(c,sql,statement,at);execute(c,sql,statement,at+4,True)
    elif kind=='extra-other-session-raw-business-UPDATE':
        business=next(e for e in events if e['kind']=='execute' and e['sql']==FIXTURE_BUSINESS);c=business['connection']
        last=max(e['qpc_ns'] for e in events if e['connection']==c and e['kind']=='dml_reply_forwarded' and e['sql']==FIXTURE_LOG)
        sql="UPDATE undo_work SET v='committed-fault-fixture' WHERE id=1";raw=packet(b'\x03'+sql.encode(),0)
        client(c,3,raw,last+1);add(c,'query',last+2,sql=sql,wire_hex=raw.hex())
    else:raise ValueError('Unknown protocol counterexample')
    events.extend(inserted);events.sort(key=lambda e:e['qpc_ns'])

def validate(wire,observation,before,after):
    mode=wire['mode'];need(mode in ['block_terminal','block_commit','drop_ok','delay_ok'] and mode==observation['mode'] and wire['claimed'] is True,'Fault role differs')
    committed=mode in ['drop_ok','delay_ok']
    need(observation['xid']=='rollback:xid' and observation['branch_id']==42 and observation['first_status']=='PhaseTwoCommitFailedRetryable','Original branch/result differs')
    need(observation['before']==[3,1,0,1,int(committed)] and observation['undo_rows_before_child']==(1 if committed else 2) and observation['after']==[3,1,0,1,1],'Independent database commit facts differ')
    need(observation['file_before']=='Pending' and observation['file_after']=='Completed' and observation['remaining_undo']==[99] and observation['business']=='committed-fault-fixture' and observation['business_replayed'] is False and observation['public_business_COMMIT'] is False,'Independent business/journal facts differ')
    child=observation['actual_commit_child'];need(child['before']=='Pending' and child['after']=='Completed' and child['branch_status']=='PhaseTwoCommitted' and child['business_replayed'] is False and child['pid']>0,'Actual recovery child facts differ')
    def text(value):
        value=value.encode('utf-8');return struct.pack('>H',len(value))+value
    original=struct.pack('>H',3)+text(observation['xid'])+struct.pack('>qB',42,0)+text(observation['resource_id'])+struct.pack('>I',0)
    records=reader.journal_records(after,original)
    first_length=struct.unpack('>I',after[8:12])[0]
    need(before==after[:12+first_length] and len(before)<len(after),'Actual Pending prefix differs from final original journal')
    events,executes,physical=base.transcript(wire)
    whole_packet(events,executes,observation,1 if committed else 2)
    auth=one(executes,'execute',AUTHORIZE);auth_rows,_,_=session(events,executes,auth['connection'],[AUTHORIZE])
    need(previous.bound_values(events,auth)==[b'rollback:xid',42],'Native authorization identity differs')
    auth_delivery=delivered(auth_rows,auth);previous.verify_control_commit(auth_rows,[auth_delivery],False,mode)
    auth_ack=one(auth_rows,'commit_reply_forwarded')
    enroll=one(executes,'execute',ENROLL)
    need(previous.bound_values(events,enroll)==[b'rollback:xid',42,observation['resource_id'].encode(),None,b'undo_work:1',1],'Actual known registration identity differs')
    terminals=[e for e in executes if e['sql']==TERMINAL]
    need(len(terminals)==(1 if committed else 2),'Actual cleanup attempts differ')
    deletes=[e for e in executes if e['sql'].startswith('DELETE ')]
    need(len(deletes)==len(terminals) and all(e['sql']==ROOT_DELETE for e in deletes),'Unexpected original deletion or business mutation')
    connections=[]
    for index,terminal in enumerate(terminals):
        need(previous.bound_values(events,terminal)==[b'rollback:xid',42],'Native terminal identity differs')
        rows,mutations,begin=session(events,executes,terminal['connection'],[ROOT_DELETE,TERMINAL],index==0 and mode=='block_terminal',index==0 and mode!='block_terminal')
        need(physical[terminal['connection']]!=physical[auth['connection']] and auth_ack['qpc_ns']<begin['qpc_ns'],'Cleanup began before distinct authorization COMMIT confirmation')
        delete=mutations[0];need(previous.bound_values(events,delete)==[b'rollback:xid',42],'Native root deletion identity differs')
        removed=delivered(rows,delete);need(removed['qpc_ns']<terminal['qpc_ns'],'Terminal attempted before original deletion ACK delivery')
        if index==0 and mode=='block_terminal':
            blocked=one(rows,'terminal_execute_blocked')
            need(blocked['sql']==TERMINAL and blocked['wire_hex']==terminal['wire_hex'] and terminal['qpc_ns']<blocked['qpc_ns'],'Blocked original terminal execute differs')
            need(not any(e['kind'] in ['commit_received','commit_forwarded','commit_reply','commit_reply_forwarded'] or (e['kind'] in ['dml_reply','dml_reply_forwarded'] and e.get('sql')==TERMINAL) for e in rows),'Blocked terminal reached execution/COMMIT')
        else:
            complete=delivered(rows,terminal)
            previous.verify_control_commit(rows,[removed,complete],index==0,mode)
            if index==0 and mode=='delay_ok':
                ack=one(rows,'commit_reply');late=[e for e in rows if e['kind'] in ['late_reply_sent','late_reply_peer_closed']]
                need(len(late)==1 and late[0]['wire_hex']==ack['wire_hex'],'Late attempt lost original server ACK bytes')
        connections.append(dict(connection=terminal['connection'],mysql_connection_id=physical[terminal['connection']]))
    need(len({e['connection'] for e in terminals})==len(terminals),'Cleanup attempts reused a physical session')
    if mode=='block_terminal':need(sum(e['kind']=='terminal_execute_blocked' for e in events)==1 and not any(e['kind']=='commit_received' and e['fault'] for e in events),'Terminal fault misclassified as COMMIT fault')
    else:need(sum(e['kind']=='commit_received' and e['fault'] for e in events)==1,'Original COMMIT fault count differs')
    return dict(verified=True,mode=mode,original_authorization_connection_id=physical[auth['connection']],actual_cleanup_connections=connections,actual_cleanup_attempts=len(terminals),journal_records=records,actual_child_pid=child['pid'],original_business_replayed=False,public_business_COMMIT=False,full_commit_gates='OPEN')

def negative_controls(wire,observation,before,after):
    cases=[]
    def reject(name,change=None,obs_change=None,old_before=None,old_after=None):
        w=copy.deepcopy(wire);o=copy.deepcopy(observation)
        if change:change(w)
        if obs_change:obs_change(o)
        try:validate(w,o,before if old_before is None else old_before,after if old_after is None else old_after)
        except (ValueError,KeyError,IndexError,struct.error):cases.append(name);return
        raise ValueError('Negative accepted: '+name)
    def target(w,sql=ROOT_DELETE):return next(e for e in w['events'] if e['kind']=='execute' and e['sql']==sql)
    def patch_parameter(w,kind):
        e=target(w);old=e['wire_hex'];raw=bytearray.fromhex(old)
        if kind=='xid':at=raw.index(b'rollback:xid');raw[at]=ord('R')
        else:raw[-8:]=struct.pack('<q',99)
        new=bytes(raw)
        for item in w['events']:
            if item['connection']==e['connection'] and item.get('wire_hex')==old:
                item['wire_hex']=new.hex()
                if item['kind']=='client_command':item['wire_sha256']=hashlib.sha256(new).hexdigest();item['wire_prefix_hex']=new[:5].hex()
    def alter_ack(w,sql,slot):
        e=target(w,sql)
        for item in w['events']:
            if item['connection']==e['connection'] and item['kind'] in ['dml_reply','dml_reply_forwarded'] and item['sql']==sql:
                raw=bytearray.fromhex(item['wire_hex']);raw[4+slot]^=1;item['wire_hex']=raw.hex()
    reject('claimed-fault-missing',lambda w:w.update(claimed=False))
    reject('different-fault-role',lambda w:w.update(mode='observe'))
    reject('original-xid-bound-byte',lambda w:patch_parameter(w,'xid'))
    reject('original-branch-bound-byte',lambda w:patch_parameter(w,'branch'))
    reject('root-delete-affected-rows',lambda w:alter_ack(w,ROOT_DELETE,1))
    reject('root-delete-lost-in-trans',lambda w:alter_ack(w,ROOT_DELETE,3))
    reject('typed-command-missing',lambda w:w['events'].remove(target(w)))
    reject('generic-command-missing',lambda w:w['events'].remove(next(e for e in w['events'] if e['kind']=='client_command' and e['connection']==target(w)['connection'] and e['wire_hex']==target(w)['wire_hex'])))
    reject('SQL-metadata-not-original',lambda w:target(w).update(sql='DELETE FROM undo_work WHERE id=?'))
    reject('wrong-observed-branch',obs_change=lambda o:o.update(branch_id=99))
    reject('wrong-independent-terminal',obs_change=lambda o:o.update(after=[3,1,0,1,0]))
    reject('wrong-independent-first-commit',obs_change=lambda o:o.update(before=[3,1,0,1,int(wire['mode'] not in ['drop_ok','delay_ok'])]))
    reject('business-changed',obs_change=lambda o:o.update(business='changed'))
    reject('other-branch-deleted',obs_change=lambda o:o.update(remaining_undo=[]))
    reject('recovery-not-completed',obs_change=lambda o:o['actual_commit_child'].update(after='Pending'))
    reject('before-journal-CRC',old_before=before[:-1]+bytes([before[-1]^1]))
    reject('after-journal-CRC',old_after=after[:-1]+bytes([after[-1]^1]))
    def reordered_auth(w):
        auth=target(w,AUTHORIZE);effect=target(w,TERMINAL)
        at=next(e['qpc_ns'] for e in w['events'] if e['connection']==effect['connection'] and e['kind']=='begin_received')
        for e in w['events']:
            if e['connection']==auth['connection'] and e['kind'] in ['commit_reply','commit_reply_forwarded']:e['qpc_ns']=at+(10 if e['kind']=='commit_reply' else 20)
        w['events'].sort(key=lambda e:e['qpc_ns'])
    reject('authorization-ACK-after-cleanup-BEGIN',reordered_auth)
    def same_physical(w):
        auth=target(w,AUTHORIZE);effect=target(w,TERMINAL)
        source=one([e for e in w['events'] if e['connection']==auth['connection']],'greeting')['mysql_connection_id']
        e=one([e for e in w['events'] if e['connection']==effect['connection']],'greeting');raw=bytearray.fromhex(e['wire_hex']);p=raw[4:];at=4+p.index(0,1)+1;raw[at:at+4]=source.to_bytes(4,'little');e['wire_hex']=raw.hex();e['mysql_connection_id']=source
    reject('authorization-and-cleanup-same-physical',same_physical)
    if wire['mode']=='block_terminal':
        reject('terminal-block-missing',lambda w:w['events'].remove(next(e for e in w['events'] if e['kind']=='terminal_execute_blocked')))
    else:
        reject('terminal-lost-in-trans',lambda w:alter_ack(w,TERMINAL,3))
        marker={'block_commit':'commit_blocked','drop_ok':'commit_reply_dropped','delay_ok':'commit_reply_held'}[wire['mode']]
        reject('actual-fault-marker-missing',lambda w:w['events'].remove(next(e for e in w['events'] if e['kind']==marker)))
    if wire['mode']=='delay_ok':
        def short_delay(w):
            held=next(e for e in w['events'] if e['kind']=='commit_reply_held')
            for e in w['events']:
                if e['kind'] in ['late_reply_sent','late_reply_peer_closed']:e['qpc_ns']=held['qpc_ns']+1000
            w['events'].sort(key=lambda e:e['qpc_ns'])
        reject('late-original-ACK-too-early',short_delay)
    for label in ['prepared-boundaries-inside','prepared-before-BEGIN-executed-inside','mid-transaction-raw-ROLLBACK','extra-enrollment-business-UPDATE','extra-other-session-raw-business-UPDATE']:
        reject(label,lambda w,label=label:injected_case(w,label))
    return dict(count=len(cases),rejected=cases)
