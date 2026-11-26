"""Independent decoding of the actual Undo DML, log deletion, fault COMMIT and explicit recovery."""
import copy
import hashlib
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'formal-undo-fence'))
from validate_wire import frame,lenenc,need
COMPENSATE='DELETE FROM `undo_work` WHERE `id` = ?'
REMOVE='DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0'
EXACT_REMOVE = REMOVE + " AND CAST(xid AS BINARY)=CAST(? AS BINARY)"
GUARD='INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))'

def root_delete(wire, xid, branch_id):
    if 'normal_delete_identity' not in wire:
        return REMOVE, [xid, branch_id]
    need(wire['normal_delete_identity'] == 'byte-exact', 'Unexpected Undo delete identity contract')
    return EXACT_REMOVE, [xid, branch_id, xid]

def parameters(event,statement,count):
    p=frame(event,0);need(p[0]==23 and int.from_bytes(p[1:5],'little')==statement and p[5]==0 and p[6:10]==b'\x01\0\0\0','Original execute header differs')
    need(count<=8 and len(p)>=12+count*2 and p[10]==0 and p[11]==1,'Original parameter flags differ');offset=12+count*2;values=[]
    for i in range(count):
        kind,flags=p[12+i*2:14+i*2];need(flags==0,'Unexpected unsigned parameter')
        if kind==8:
            need(offset+8<=len(p),'Truncated native long');values.append(int.from_bytes(p[offset:offset+8],'little',signed=True));offset+=8
        else:
            need(kind in (252,253,254),'Unexpected native value type');n,offset=lenenc(p,offset);need(n<=128 and offset+n<=len(p),'Native string budget differs');values.append(p[offset:offset+n]);offset+=n
    need(offset==len(p),'Trailing parameter bytes');return values

def validate(wire,observation):
    mode=wire['mode'];events=wire['events'];need(mode in ('block_commit','drop_ok','delay_ok') and wire['claimed'] is True,'Fault mode differs')
    need([e['qpc_ns'] for e in events]==sorted(e['qpc_ns'] for e in events) and not any(e['kind']=='observer_error' for e in events),'Observer ordering/error')
    connections={};statements={};dml=[];requests=[];forwards=[];replies=[];queries=[];executions=[]
    remove, identity = root_delete(wire, b'rollback:xid', 42)
    expected_values={COMPENSATE:[1],remove:identity,GUARD:[42,b'rollback:xid',b'serializer=jackson&compressorType=NONE',b'{}']}
    for e in events:
        kind=e['kind'];connection=e['connection']
        if kind=='greeting':
            p=frame(e,0);need(p[0]==10,'Greeting version');end=p.index(0,1);identity=int.from_bytes(p[end+1:end+5],'little');need(identity==e['mysql_connection_id'] and identity>0 and connection not in connections,'Physical MySQL connection differs');connections[connection]=identity
        elif kind in ('query','prepare'):
            p=frame(e,0);need(p[0]==(3 if kind=='query' else 22) and p[1:].decode()==e['sql'],'Original SQL command differs')
            if kind=='query' and p==b'\x03COMMIT':queries.append(e)
        elif kind=='prepare_ok':
            p=frame(e,1);need(len(p)==12 and p[0]==0,'Original prepare metadata differs');count=int.from_bytes(p[7:9],'little');statement=int.from_bytes(p[1:5],'little');statements[connection,statement]=(e['sql'],count)
            if e['sql'] in expected_values:need(count==len(expected_values[e['sql']]) and p[5:7]==b'\0\0','Original DML prepare metadata differs')
            need(any(v['kind']=='prepare' and v['connection']==connection and v['sql']==e['sql'] and v['qpc_ns']<e['qpc_ns'] for v in events),'Original prepare request missing')
        elif kind=='execute':
            p=frame(e,0);need(len(p)>=10 and p[0]==23 and p[5]==0 and p[6:10]==b'\x01\0\0\0','Original execute command header differs');statement=int.from_bytes(p[1:5],'little');need((connection,statement) in statements,'Original DML prepare binding missing');sql,count=statements[connection,statement]
            need(sql==e['sql'],'Original executed statement SQL differs');executions.append(e)
            if sql.startswith(('DELETE ','UPDATE ','INSERT ')):
                need(sql in expected_values and parameters(e,statement,count)==expected_values[sql],'Original Undo or identity parameters differ');dml.append(e)
        elif kind in ('commit_received','commit_forwarded'):
            need(frame(e,0)==b'\x03COMMIT','Original COMMIT bytes differ');(requests if kind=='commit_received' else forwards).append(e)
        elif kind=='commit_reply':
            p=frame(e,1);need(len(p)>=7 and p[0:3]==b'\0\0\0' and int.from_bytes(p[3:5],'little')&1==0 and p.hex()==e['payload_hex'] and hashlib.sha256(p).hexdigest()==e['sha256'] and e['ok'] is True,'Original completed COMMIT OK differs');replies.append(e)
    need(len(requests)==len(queries)==2 and len(forwards)==(1 if mode=='block_commit' else 2) and len(replies)==(1 if mode=='block_commit' else 2),'Actual control COMMIT counts differ')
    need(len({e['connection'] for e in requests})==2 and all(e['connection'] in connections for e in requests),'Original COMMIT physical sessions differ')
    for e in requests:
        need(sum(q['connection']==e['connection'] and q['qpc_ns']<e['qpc_ns'] for q in queries)==1,'Original COMMIT query missing')
        session=[v for v in events if v['connection']==e['connection']]
        commands=[v for v in session if v['kind']=='client_command'];need(commands,'Complete original client command stream missing')
        for command in commands:
            prefix=bytes.fromhex(command['wire_prefix_hex']);need(len(prefix)==5 and prefix[3]==0 and prefix[4]==command['opcode'] and command['opcode'] in (1,3,14,22,23,25),'Unsafe original client command opcode')
            p=frame(command,0);need(p and p[0]==command['opcode'] and hashlib.sha256(bytes.fromhex(command['wire_hex'])).hexdigest()==command['wire_sha256'],'Original client command bytes/hash differ')
            if command['opcode'] in (3,22,23):
                tag={3:'query',22:'prepare',23:'execute'}[command['opcode']];need(any(v['kind']==tag and v['wire_hex']==command['wire_hex'] and v['qpc_ns']>command['qpc_ns'] for v in session),'Client command lacks exact original typed request')
        for request in session:
            if request['kind'] in ('query','prepare','execute'):need(any(command['wire_hex']==request['wire_hex'] and command['qpc_ns']<request['qpc_ns'] for command in commands),'Typed request lacks original client command')
        transaction_queries=[v for v in session if v['kind']=='query' and v['sql'].strip().upper() in ('BEGIN','COMMIT')]
        need([v['sql'] for v in transaction_queries]==['BEGIN','COMMIT'],'Original single transaction command sequence differs')
        begin=transaction_queries[0]
        need(all(command['opcode']!=1 for command in commands if begin['qpc_ns']<command['qpc_ns']<e['qpc_ns']),'QUIT inside original Undo transaction')
        need([v['sql'] for v in session if v['kind']=='query' and begin['qpc_ns']<=v['qpc_ns']<=e['qpc_ns']]==['BEGIN','COMMIT'],'Unexpected transaction boundary inside Undo')
        for v in session:
            if v['kind']=='prepare' and begin['qpc_ns']<v['qpc_ns']<e['qpc_ns']:
                need((v['sql'].startswith('SELECT ') or v['sql']=='SHOW GRANTS' or v['sql'] in expected_values) and not any(token in v['sql'] for token in (';','/*','--','#')),'Unsafe prepared command inside Undo transaction')
        for executed in executions:
            if executed['connection']==e['connection'] and begin['qpc_ns']<executed['qpc_ns']<e['qpc_ns']:
                need((executed['sql'].startswith('SELECT ') or executed['sql']=='SHOW GRANTS' or executed['sql'] in expected_values) and not any(token in executed['sql'] for token in (';','/*','--','#')),'Unsafe executed statement inside Undo transaction')
        started=[v for v in session if v['kind']=='begin_received'];sent=[v for v in session if v['kind']=='begin_forwarded'];acks=[v for v in session if v['kind']=='begin_reply'];delivered=[v for v in session if v['kind']=='begin_reply_forwarded']
        need(len(started)==len(sent)==len(acks)==len(delivered)==1 and frame(started[0],0)==frame(sent[0],0)==b'\x03BEGIN' and begin['qpc_ns']<started[0]['qpc_ns']<sent[0]['qpc_ns']<acks[0]['qpc_ns']<delivered[0]['qpc_ns'],'Original BEGIN request/reply ordering differs')
        ack=frame(acks[0],1);need(len(ack)>=7 and ack[:3]==b'\0\0\0' and int.from_bytes(ack[3:5],'little')&1==1 and delivered[0]['wire_hex']==acks[0]['wire_hex'],'Original BEGIN in-transaction OK differs')
        if not e['fault']:
            acks=[a for a in replies if a['connection']==e['connection']];sent_acks=[a for a in session if a['kind']=='commit_reply_forwarded'];need(len(acks)==len(sent_acks)==1 and acks[0]['qpc_ns']>e['qpc_ns'] and sent_acks[0]['qpc_ns']>acks[0]['qpc_ns'] and frame(sent_acks[0],1)==frame(acks[0],1) and sent_acks[0]['wire_hex']==acks[0]['wire_hex'],'Explicit recovery original COMMIT OK forwarding differs')
        prior=[d for d in dml if d['connection']==e['connection'] and d['qpc_ns']<e['qpc_ns']]
        need(all(d['qpc_ns']>delivered[0]['qpc_ns'] for d in prior),'Undo DML outside original BEGIN/COMMIT')
        expected=[GUARD] if mode!='block_commit' and not e['fault'] else [COMPENSATE,remove]
        need([d['sql'] for d in prior]==expected,'Compensation/delete/fence order differs')
    fault=[e for e in requests if e['fault']];need(len(fault)==1,'Exactly one control fault required');fault=fault[0];connection=fault['connection'];actions=[e for e in events if e['connection']==connection]
    if mode=='block_commit':
        need(not any(e['connection']==connection for e in forwards) and sum(e['kind']=='commit_blocked' and e['qpc_ns']>fault['qpc_ns'] for e in actions)==1,'Blocked COMMIT was forwarded')
    else:
        forwarded=[e for e in forwards if e['connection']==connection];fault_replies=[e for e in replies if e['connection']==connection];need(len(forwarded)==len(fault_replies)==1 and fault['qpc_ns']<forwarded[0]['qpc_ns']<fault_replies[0]['qpc_ns'],'Fault COMMIT/OK connection or order differs')
        action='commit_reply_dropped' if mode=='drop_ok' else 'commit_reply_held';held=[e for e in actions if e['kind']==action];need(len(held)==1 and held[0]['qpc_ns']>fault_replies[0]['qpc_ns'] and not any(e['kind']=='commit_reply_forwarded' for e in actions),'Fault reply action differs')
        if mode=='delay_ok':
            late=[e for e in actions if e['kind'] in ('late_reply_sent','late_reply_peer_closed')];need(len(late)==1 and late[0]['qpc_ns']-held[0]['qpc_ns']>=2_100_000_000,'Actual reply hold too short')
    need(len(dml)==(4 if mode=='block_commit' else 3),'Unexpected retry/business DML')
    o=observation;need(o['mode']==mode and o['unknown'] is True and o['original_xid']=='rollback:xid' and o['original_branch_id']==42 and o['before_explicit_recovery']==dict(business=int(mode=='block_commit'),undo=int(mode=='block_commit')) and o['after_explicit_recovery']==dict(business=0,undo=int(mode!='block_commit')),'Actual original identity or database state differs')
    return dict(verified=True,mode=mode,complete_original_client_command_stream_verified=True,original_single_BEGIN_COMMIT_transactions_verified=2,original_in_transaction_BEGIN_OK_verified=2,decoded_original_commit_requests=2,actual_forwarded_commits=len(forwards),decoded_compensation_DML=sum(e['sql']==COMPENSATE for e in dml),decoded_original_log_deletes=sum(e['sql']==remove for e in dml),decoded_guard_inserts=sum(e['sql']==GUARD for e in dml),original_xid='rollback:xid',original_branch_id=42,physical_MySQL_ids=[connections[e['connection']] for e in requests],automatic_retries=0,original_business_replays=0)

def negative_controls(wire,observation):
    remove, _ = root_delete(wire, b'rollback:xid', 42)
    controls=[]
    def packet(payload,seq=0):return (len(payload).to_bytes(3,'little')+bytes([seq])+payload).hex()
    def link(changed,event,new=False):
        raw=bytes.fromhex(event['wire_hex']);previous=[v for v in changed['events'] if v['kind']=='client_command' and v['connection']==event['connection'] and v['qpc_ns']<event['qpc_ns']]
        command=dict(kind='client_command',connection=event['connection'],qpc_ns=event['qpc_ns']-1,opcode=raw[4],wire_prefix_hex=raw[:5].hex(),wire_sha256=hashlib.sha256(raw).hexdigest(),wire_hex=raw.hex())
        if new:changed['events'].append(command)
        else:
            target=max(previous,key=lambda v:v['qpc_ns']);target.update(command)
    for name in ['wrong_bind_identity','wrong_compensation_primary_key','wrong_commit_bytes','wrong_connection','missing_prepare','forwarded_ERR_masquerades_as_OK','intermediate_ROLLBACK','prepared_autocommit_boundary','preprepared_autocommit_execution','reset_connection_inside_transaction']:
        changed=copy.deepcopy(wire)
        if name=='wrong_bind_identity':
            e=next(e for e in changed['events'] if e['kind']=='execute' and e.get('sql')==remove);e['wire_hex']=e['wire_hex'].replace(b'rollback:xid'.hex(),b'rollback:bad'.hex());link(changed,e)
        elif name=='wrong_compensation_primary_key':
            e=next(e for e in changed['events'] if e['kind']=='execute' and e.get('sql')==COMPENSATE);p=bytearray.fromhex(e['wire_hex']);p[-8:]=(2).to_bytes(8,'little',signed=True);e['wire_hex']=p.hex();link(changed,e)
        elif name=='wrong_commit_bytes':
            e=next(e for e in changed['events'] if e['kind']=='commit_received');e['wire_hex']=e['wire_hex'].replace(b'COMMIT'.hex(),b'BOGUS!'.hex())
        elif name=='wrong_connection':next(e for e in changed['events'] if e['kind']=='commit_received')['connection']=99999
        elif name=='missing_prepare':changed['events']=[e for e in changed['events'] if e['kind']!='prepare_ok']
        elif name=='forwarded_ERR_masquerades_as_OK':
            e=next(e for e in changed['events'] if e['kind']=='commit_reply_forwarded');p=bytearray.fromhex(e['wire_hex']);p[4]=255;e['wire_hex']=p.hex()
        else:
            first=next(e for e in changed['events'] if e['kind']=='execute' and e.get('sql')==COMPENSATE);e=dict(connection=first['connection'],qpc_ns=first['qpc_ns']+2)
            if name=='intermediate_ROLLBACK':e.update(kind='query',sql='ROLLBACK',wire_hex=packet(b'\x03ROLLBACK'));changed['events'].append(e);link(changed,e,True)
            elif name=='prepared_autocommit_boundary':e.update(kind='prepare',sql='SET autocommit=1',wire_hex=packet(b'\x16SET autocommit=1'));changed['events'].append(e);link(changed,e,True)
            elif name=='preprepared_autocommit_execution':
                begin=next(v for v in changed['events'] if v['kind']=='begin_received' and v['connection']==first['connection']);statement=1234567
                prepare=dict(kind='prepare',connection=first['connection'],qpc_ns=begin['qpc_ns']-100000,sql='SET autocommit=1',wire_hex=packet(b'\x16SET autocommit=1'));changed['events'].append(prepare);link(changed,prepare,True)
                changed['events'].append(dict(kind='prepare_ok',connection=first['connection'],qpc_ns=prepare['qpc_ns']+10,sql='SET autocommit=1',wire_hex=packet(b'\0'+statement.to_bytes(4,'little')+b'\0'*7,1)))
                e.update(kind='execute',sql='SET autocommit=1',statement=statement,wire_hex=packet(b'\x17'+statement.to_bytes(4,'little')+b'\0\x01\0\0\0'));changed['events'].append(e);link(changed,e,True)
            else:
                raw=bytes.fromhex(packet(b'\x1f'));e.update(kind='client_command',opcode=31,wire_hex=raw.hex(),wire_prefix_hex=raw[:5].hex(),wire_sha256=hashlib.sha256(raw).hexdigest());changed['events'].append(e)
            changed['events'].sort(key=lambda v:v['qpc_ns'])
        try:validate(changed,observation)
        except ValueError as error:controls.append(dict(control=name,rejection=str(error)))
        else:raise ValueError('Altered actual rollback evidence accepted: '+name)
    return controls
