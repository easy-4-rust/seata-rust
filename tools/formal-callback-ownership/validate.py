"""Independent raw binding and transaction checks for callback-origin persistence, not Java parity."""
import copy
import hashlib
import importlib.util
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('callback_mysql_transcript',ROOT/'tools/formal-rm-mysql/validate.py')
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
base.EXPECTED={}
need,frame,lenenc,ok_packet=base.need,base.frame,base.lenenc,base.ok_packet
SOURCE='INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,application_data,lock_key,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(2,?,?,?,?,NULL,2,?,?,0,now(6),now(6))'
GUARD='INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))'
TERMINAL=base.TERMINAL

def parameters(event,count):
    p=frame(event,0);need(0<count<=8 and len(p)>=12+2*count and p[11]==1,'Native parameter shape differs')
    null=p[10];offset=12+count*2;values=[]
    for i in range(count):
        kind,flags=p[12+2*i:14+2*i];need(flags in (0,128),'Parameter flag differs')
        if null&(1<<i):values.append(None);continue
        if kind in (1,2,3,8):
            width={1:1,2:2,3:4,8:8}[kind];need(offset+width<=len(p),'Truncated integer');values.append(int.from_bytes(p[offset:offset+width],'little',signed=flags==0));offset+=width
        else:
            need(kind in (252,253,254) and flags==0,'Native string type differs');n,offset=lenenc(p,offset);need(n<=32767 and offset+n<=len(p),'Native string budget differs');values.append(p[offset:offset+n]);offset+=n
    need(offset==len(p),'Trailing parameter bytes');return values

def validate(wire,observation):
    mode=wire['mode'];need(mode in ('block_commit','drop_ok','delay_ok') and observation['mode']==mode and wire['claimed'] is True,'Fault role differs')
    need(observation['first']=='Pending' and observation['recovered']=='Completed' and observation['after']==[1,2,1] and observation['before']==([0,0,0] if mode=='block_commit' else [1,2,0]),'Independent database/journal facts differ')
    need(observation['original_register_requests']==0 and observation['business_replayed'] is False and observation['version']==2 and observation['origin']==2 and observation['lock_key'] is None,'Unknown-origin scope differs')
    events,executes,physical=base.transcript(wire)
    expected={SOURCE:[observation['xid'].encode(),observation['branch_id'],observation['resource_id'].encode(),None,1,2],GUARD:[observation['branch_id'],observation['xid'].encode(),b'serializer=jackson&compressorType=NONE',b'{}'],TERMINAL:[observation['xid'].encode(),observation['branch_id']]}
    dml=[e for e in executes if e['sql'].startswith(('INSERT ','UPDATE ','DELETE '))]
    for e in dml:
        need(e['sql'] in expected,'Unexpected business or control DML')
        prepare=[v for v in events if v['kind']=='prepare_ok' and v['connection']==e['connection'] and int.from_bytes(frame(v,1)[1:5],'little')==e['statement'] and v['qpc_ns']<e['qpc_ns']]
        need(len(prepare)==1,'Unique prepare binding required');count=int.from_bytes(frame(prepare[0],1)[7:9],'little')
        need(parameters(e,count)==expected[e['sql']],'Original identity/profile/NULL parameters differ')
    sources=[e for e in dml if e['sql']==SOURCE];effects=[e for e in dml if e['sql']!=SOURCE]
    need(len(sources)==(2 if mode=='block_commit' else 1) and [e['sql'] for e in effects]==[GUARD,TERMINAL],'Unexpected replay or effect order')
    need(effects[0]['connection']==effects[1]['connection'] and len({e['connection'] for e in sources+effects})==len(sources)+1,'Source and guarded effect must use separate physical transactions')
    fault=[e for e in events if e['kind']=='commit_received' and e['fault'] is True];need(len(fault)==1 and fault[0]['connection']==sources[0]['connection'],'Exactly the first origin COMMIT must be faulted')
    for c in {e['connection'] for e in dml}:
        session=[e for e in events if e['connection']==c];local=[e for e in dml if e['connection']==c];is_fault=c==fault[0]['connection']
        begin=[e for e in session if e['kind']=='begin_received'];acks=[e for e in session if e['kind']=='begin_reply'];delivered=[e for e in session if e['kind']=='begin_reply_forwarded'];forward=[e for e in session if e['kind']=='begin_forwarded']
        need(len(begin)==len(acks)==len(delivered)==len(forward)==1,'Unique original BEGIN/forward/ACK required');ok_packet(acks[0],True,0)
        typed=[e for e in session if e['kind']=='query' and e['sql']=='BEGIN'];need(len(typed)==1 and typed[0]['qpc_ns']<begin[0]['qpc_ns'] and frame(typed[0],0)==frame(begin[0],0)==frame(forward[0],0)==b'\x03BEGIN' and acks[0]['wire_hex']==delivered[0]['wire_hex'],'Original BEGIN request/forward/ACK bytes differ')
        need(begin[0]['qpc_ns']<forward[0]['qpc_ns']<acks[0]['qpc_ns']<delivered[0]['qpc_ns']<local[0]['qpc_ns'],'DML must follow delivered transactional BEGIN ACK')
        boundaries=[e['sql'] for e in session if e['kind']=='query' and e['sql'] in ('BEGIN','COMMIT','ROLLBACK')]
        need(boundaries in ([['BEGIN','COMMIT'],['BEGIN','COMMIT','ROLLBACK']] if is_fault else [['BEGIN','COMMIT']]),'Control transaction split or cleanup order differs')
        for e in session:
            if e['qpc_ns']<begin[0]['qpc_ns']:continue
            if e['kind'] in ('query','prepare','execute'):
                sql=e['sql'];need(';' not in sql and '/*' not in sql and '--' not in sql,'SQL boundary escape')
                need(sql in expected or sql=='SHOW GRANTS' or sql.startswith('SELECT ') or (e['kind']=='query' and sql in ('BEGIN','COMMIT','ROLLBACK')),'Unclassified control SQL')
            if e['kind']=='client_command':need(e['opcode'] in (1,3,22,23,25),'Unexpected protocol command')
        replies=[]
        for e in local:
            ack=[v for v in session if v['kind']=='dml_reply' and v['sql']==e['sql'] and v['qpc_ns']>e['qpc_ns']];need(len(ack)==1,'Unique DML result required');ok_packet(ack[0],True,1);replies+=ack
        commits=[e for e in session if e['kind']=='commit_received'];need(len(commits)==1 and commits[0]['qpc_ns']>max(e['qpc_ns'] for e in replies) and commits[0]['fault']==is_fault,'COMMIT provenance/order differs')
        forwards=[e for e in session if e['kind']=='commit_forwarded'];acks=[e for e in session if e['kind']=='commit_reply'];delivered=[e for e in session if e['kind']=='commit_reply_forwarded']
        if is_fault and mode=='block_commit':need(not forwards and not acks and not delivered and sum(e['kind']=='commit_blocked' for e in session)==1,'Blocked COMMIT reached server')
        else:
            need(len(forwards)==len(acks)==1 and commits[0]['qpc_ns']<forwards[0]['qpc_ns']<acks[0]['qpc_ns'],'Actual server COMMIT ACK required');ok_packet(acks[0],False,0)
            need(len(delivered)==(0 if is_fault else 1),'Unconfirmed ACK delivery differs')
            if is_fault:need(sum(e['kind']==('commit_reply_held' if mode=='delay_ok' else 'commit_reply_dropped') for e in session)==1,'ACK fault missing')
            if is_fault and mode=='delay_ok':
                held=[e for e in session if e['kind']=='commit_reply_held'];late=[e for e in session if e['kind'] in ['late_reply_sent','late_reply_peer_closed']]
                need(len(held)==len(late)==1 and acks[0]['qpc_ns']<held[0]['qpc_ns']<late[0]['qpc_ns'] and late[0]['qpc_ns']-held[0]['qpc_ns']>=2_100_000_000,'Actual late ACK must exceed the 1500ms client budget')
    need(fault[0]['qpc_ns']<effects[0]['qpc_ns'],'Effect ran before unknown registration was retried')
    return dict(verified=True,mode=mode,raw_commands=sum(e['kind']=='client_command' for e in events),physical_connections=len(physical),source_inserts=len(sources),guarded_terminal_transactions=1,unknown_lock_knowledge=True)

def negative_controls(wire,observation):
    rejected=[]
    def reject(name,w,o):
        try:validate(w,o)
        except (ValueError,KeyError,IndexError):rejected.append(name);return
        raise ValueError('Mutation accepted: '+name)
    for kind in ['client_command','prepare_ok','begin_reply','begin_reply_forwarded','dml_reply','commit_received']:
        changed=copy.deepcopy(wire);index=next(i for i,e in enumerate(changed['events']) if e['kind']==kind);changed['events'].pop(index);reject('missing-'+kind,changed,observation)
    for name,key,value in [('wrong-origin','origin',1),('unknown-treated-as-none','lock_key',''),('fabricated-phase-one','original_register_requests',1),('false-completed','first','Completed'),('wrong-branch','branch_id',42),('wrong-resource','resource_id','other')]:
        changed=copy.deepcopy(observation);changed[key]=value;reject(name,wire,changed)
    changed=copy.deepcopy(wire);next(e for e in changed['events'] if e['kind']=='commit_received' and e['fault'])['fault']=False;reject('fault-role-unbound',changed,observation)
    changed=copy.deepcopy(wire);next(e for e in changed['events'] if e['kind']=='begin_forwarded')['wire_hex']='0700000003434f4d4d4954';reject('forwarded-BEGIN-replaced-with-COMMIT',changed,observation)
    if wire['mode']=='delay_ok':
        changed=copy.deepcopy(wire);changed['events']=[e for e in changed['events'] if e['kind'] not in ['late_reply_sent','late_reply_peer_closed']];reject('missing-actual-late-ACK',changed,observation)
        changed=copy.deepcopy(wire);held=next(e for e in changed['events'] if e['kind']=='commit_reply_held');next(e for e in changed['events'] if e['kind'] in ['late_reply_sent','late_reply_peer_closed'])['qpc_ns']=held['qpc_ns']+1;changed['events'].sort(key=lambda e:e['qpc_ns']);reject('ACK-not-actually-delayed',changed,observation)
    return dict(all_rejected=True,count=len(rejected),cases=rejected)
