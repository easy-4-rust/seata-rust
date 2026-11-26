"""Independent bounded decoder of archived MySQL request/prepare/execute/COMMIT frames; fixture only."""
import copy
import hashlib

INSERT="INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))"
def need(value,message):
    if not value:raise ValueError(message)
def frame(event,sequence=None):
    wire=bytes.fromhex(event['wire_hex']);need(len(wire)>=4 and int.from_bytes(wire[:3],'little')==len(wire)-4,'Malformed original packet length')
    if sequence is not None:need(wire[3]==sequence,'Original packet sequence differs')
    return wire[4:]
def lenenc(data,offset):
    need(offset<len(data),'Missing lenenc')
    first=data[offset];offset+=1
    if first<251:return first,offset
    need(first in (252,253,254),'Unsupported lenenc')
    width={252:2,253:3,254:8}[first];need(offset+width<=len(data),'Truncated lenenc');return int.from_bytes(data[offset:offset+width],'little'),offset+width
def parameters(event,statement):
    p=frame(event,0);need(len(p)>=20 and p[0]==23 and int.from_bytes(p[1:5],'little')==statement and p[5]==0 and p[6:10]==b'\x01\0\0\0','Original execute header differs')
    need(p[10]==0 and p[11]==1,'Original execute null/type flags differ')
    types=[(p[12+i*2],p[13+i*2]) for i in range(4)]
    need(types[0]==(8,0) and all(t in [(253,0),(254,0),(252,0)] for t in types[1:]),'Original execute parameter types differ')
    offset=20;need(len(p)>=offset+8,'Missing branch long');branch=int.from_bytes(p[offset:offset+8],'little',signed=True);offset+=8;values=[]
    for _ in range(3):
        length,offset=lenenc(p,offset);need(length<=128 and offset+length<=len(p),'Original execute string budget differs');values.append(p[offset:offset+length]);offset+=length
    need(offset==len(p),'Original execute trailing bytes')
    return branch,values
def validate(wire,observation):
    events=wire['events'];mode=wire['mode'];need(mode in ('drop_ok','block_commit','delay_ok') and wire['claimed'] is True,'Fault mode/claim differs')
    need(not any(e['kind']=='observer_error' for e in events),'Observer error')
    need([e['qpc_ns'] for e in events]==sorted(e['qpc_ns'] for e in events),'Actual observer ordering differs')
    connections={};prepared={};inserts=[];commits=[];queries=[];forwards=[];replies=[]
    for e in events:
        key=e['connection'];kind=e['kind']
        if kind=='greeting':
            p=frame(e,0);need(p and p[0]==10,'Original greeting version differs');end=p.index(0,1);mysql=int.from_bytes(p[end+1:end+5],'little');need(mysql>0 and mysql==e['mysql_connection_id'] and key not in connections,'Original physical connection differs');connections[key]=mysql
        elif kind in ('query','prepare'):
            p=frame(e,0);need(p and p[0]==(3 if kind=='query' else 22) and p[1:].decode('utf-8')==e['sql'],'Original SQL command differs')
            if kind=='query' and p==b'\x03COMMIT':queries.append(e)
        elif kind=='prepare_ok' and e['sql']==INSERT:
            p=frame(e,1);need(len(p)==12 and p[0]==0 and p[5:7]==b'\0\0' and int.from_bytes(p[7:9],'little')==4,'Original prepare metadata differs');statement=int.from_bytes(p[1:5],'little')
            need(any(x['kind']=='prepare' and x['connection']==key and x['sql']==INSERT and x['qpc_ns']<e['qpc_ns'] for x in events),'Original prepare request missing');prepared[key,statement]=e
        elif kind=='execute' and e.get('wire_hex'):
            p=frame(e,0);statement=int.from_bytes(p[1:5],'little');need(e['sql']==INSERT and (key,statement) in prepared and prepared[key,statement]['qpc_ns']<e['qpc_ns'],'Original insert statement binding differs')
            branch,values=parameters(e,statement)
            need(branch==4242 and values==[b'fence:wire',b'serializer=jackson&compressorType=NONE',b'{}'],'Original insert identity/content differs');inserts.append(e)
        elif kind in ('commit_received','commit_forwarded'):
            need(frame(e,0)==b'\x03COMMIT','Original COMMIT request differs')
            (commits if kind=='commit_received' else forwards).append(e)
        elif kind=='commit_reply':
            p=frame(e,1);need(len(p)>=7 and p[0]==0 and p[1:3]==b'\0\0' and p.hex()==e['payload_hex'] and hashlib.sha256(p).hexdigest()==e['sha256'] and e['ok'] is True,'Original COMMIT OK differs')
            # OK状态位SERVER_STATUS_IN_TRANS必须清除；不能把尚在事务中的回复充当提交确认。
            need(int.from_bytes(p[3:5],'little')&1==0,'COMMIT OK still in transaction');replies.append(e)
    expected=2 if mode=='block_commit' else 1
    need(len(commits)==len(queries)==len(inserts)==expected and len(forwards)==1 and len(replies)==(0 if mode=='block_commit' else 1),'Original request/reply counts differ')
    need(len({connections[e['connection']] for e in inserts})==expected,'Original physical insert sessions differ')
    for c in commits:
        key=c['connection'];need(key in connections,'COMMIT physical connection missing')
        need(sum(e['connection']==key and e['qpc_ns']<c['qpc_ns'] for e in inserts)==1,'COMMIT lacks exact preceding original insert')
        need(sum(e['connection']==key and e['qpc_ns']<c['qpc_ns'] for e in queries)==1,'COMMIT raw request missing')
    fault=[e for e in commits if e['fault'] is True];need(len(fault)==1,'Original fault count differs');fault=fault[0];key=fault['connection']
    target_forwards=[e for e in forwards if e['connection']==key]
    target_actions=[e for e in events if e['connection']==key]
    if mode=='block_commit':
        need(not target_forwards and sum(e['kind']=='commit_blocked' and e['qpc_ns']>fault['qpc_ns'] for e in target_actions)==1,'Blocked COMMIT was forwarded')
    else:
        need(len(target_forwards)==len(replies)==1 and replies[0]['connection']==key and fault['qpc_ns']<target_forwards[0]['qpc_ns']<replies[0]['qpc_ns'],'Original fault COMMIT ordering/connection differs')
        need(not any(e['kind']=='commit_reply_forwarded' for e in target_actions),'Fault reply was forwarded')
        action='commit_reply_dropped' if mode=='drop_ok' else 'commit_reply_held'
        actions=[e for e in target_actions if e['kind']==action];need(len(actions)==1 and actions[0]['qpc_ns']>replies[0]['qpc_ns'],'Original fault reply action differs')
        if mode=='delay_ok':
            late=[e for e in target_actions if e['kind'] in ('late_reply_sent','late_reply_peer_closed')];need(len(late)==1 and late[0]['qpc_ns']-actions[0]['qpc_ns']>=1_100_000_000,'Actual reply hold was too short')
    o=observation;need(o['mode']==mode and o['unknown'] is True and o['xid']=='fence:wire' and o['branch_id']==4242 and o['before_explicit_recovery']==int(mode!='block_commit') and o['recovery_inserted']==(mode=='block_commit') and o['after']==1 and o['business_rows']==0,'Actual client/database observation differs')
    return dict(verified=True,mode=mode,decoded_original_guard_inserts=len(inserts),decoded_original_commit_requests=len(commits),actual_forwarded_commits=len(forwards),decoded_original_commit_OK=len(replies),original_xid='fence:wire',original_branch_id=4242,physical_mysql_connection_ids=[connections[e['connection']] for e in inserts],automatic_retries=0)
def negative_controls(wire,observation):
    controls=[]
    def reject(name,changed):
        try:validate(changed,observation)
        except ValueError as error:controls.append(dict(control=name,rejection=str(error)))
        else:raise ValueError('Altered actual wire accepted: '+name)
    changed=copy.deepcopy(wire);e=next(e for e in changed['events'] if e['kind']=='execute' and e.get('wire_hex'));e['wire_hex']=e['wire_hex'].replace(b'fence:wire'.hex(),b'fence:wirX'.hex());reject('wrong_original_bind_xid',changed)
    changed=copy.deepcopy(wire);e=next(e for e in changed['events'] if e['kind']=='commit_received');e['wire_hex']=e['wire_hex'].replace(b'COMMIT'.hex(),b'BOGUS!'.hex());reject('non_COMMIT_raw_request',changed)
    changed=copy.deepcopy(wire);next(e for e in changed['events'] if e['kind']=='commit_received')['connection']=99999;reject('wrong_physical_connection',changed)
    changed=copy.deepcopy(wire);changed['events']=[e for e in changed['events'] if e['kind']!='prepare_ok'];reject('missing_original_prepare_binding',changed)
    changed=copy.deepcopy(wire)
    if wire['mode']=='block_commit':
        e=copy.deepcopy(next(e for e in changed['events'] if e['kind']=='commit_received' and e['fault']));e['kind']='commit_forwarded';changed['events'].append(e);changed['events'].sort(key=lambda e:e['qpc_ns']);reject('blocked_request_actually_forwarded',changed)
    else:
        e=copy.deepcopy(next(e for e in changed['events'] if e['kind']=='commit_reply'));e['kind']='commit_reply_forwarded';changed['events'].append(e);changed['events'].sort(key=lambda e:e['qpc_ns']);reject('dropped_or_held_reply_actually_forwarded',changed)
    return controls
