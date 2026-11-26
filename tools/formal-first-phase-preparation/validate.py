"""Independent original MySQL frames: business and Normal Undo share BEGIN/ROLLBACK, ownership commits separately."""
import copy
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
base=module('preparation_raw_transcript',ROOT/'tools/formal-rm-mysql/validate.py');base.EXPECTED={}
native=module('preparation_native_parameters',ROOT/'tools/formal-callback-ownership/validate.py')
frame,need,ok_packet=base.frame,base.need,base.ok_packet
ENROLL='INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,application_data,lock_key,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(2,?,?,?,?,?,1,?,0,0,now(6),now(6))'
NORMAL='INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,0,now(6),now(6))'
TEMP_CREATE='CREATE TEMPORARY TABLE `__seata_at_keys_1` (`r` BIGINT UNSIGNED NOT NULL PRIMARY KEY, `k0` INT NOT NULL) ENGINE=InnoDB'
TEMP_INSERT='INSERT INTO `__seata_at_keys_1` VALUES (0, (1))'
TEMP_DROP='DROP TEMPORARY TABLE `__seata_at_keys_1`'
TEMP=[TEMP_CREATE,TEMP_INSERT,TEMP_DROP]


def validate(wire,observation,warning_probe):
    warning_rows=[line.split("\t") for line in warning_probe.splitlines() if line]
    need(len(warning_rows)==2 and all(row[0]=="Warning" for row in warning_rows) and sorted(int(row[1]) for row in warning_rows)==[1751,1752],"Independent temporary DDL rollback warning probe differs")
    need(wire['mode']=='observe' and wire['claimed'] is False,'Unexpected SQL fault role')
    events,executes,physical=base.transcript(wire)
    mutations=[e for e in executes if e['sql'].startswith(('INSERT ','UPDATE ','DELETE ')) or e['sql'] in TEMP]
    need([e['sql'] for e in mutations[:3]]==TEMP and len(mutations)==6,'Exact internal temporary-key staging lifecycle required')
    dml=mutations[3:]
    need(len(dml)==3 and dml[1]['sql']==ENROLL and dml[2]['sql']==NORMAL,'Exactly one business write, ownership enrollment and Normal Undo required')
    business,ownership,undo=dml
    need(business['sql']=="INSERT INTO undo_work(id,v) VALUES(1,'prepare_not_committed')",'Original business SQL differs')
    need(all(e['connection']==business['connection'] for e in mutations[:3]) and business['connection']==undo['connection']!=ownership['connection'],'Business and Undo must share original physical connection, ownership must be separate')
    expected={ENROLL:[observation['xid'].encode(),observation['branch_id'],observation['resource_id'].encode(),b'original-prepare-data',b'undo_work:1',1]}
    for e in [ownership,undo]:
        prepare=[v for v in events if v['kind']=='prepare_ok' and v['connection']==e['connection'] and int.from_bytes(frame(v,1)[1:5],'little')==e['statement'] and v['qpc_ns']<e['qpc_ns']];need(len(prepare)==1,'Actual unique prepare binding required')
        count=int.from_bytes(frame(prepare[0],1)[7:9],'little');values=native.parameters(e,count)
        if e['sql']==ENROLL:need(values==expected[ENROLL],'Original enrollment XID/branch/resource/app/lock/profile differs')
        else:
            need(len(values)==4 and values[:3]==[observation['branch_id'],observation['xid'].encode(),b'serializer=jackson&compressorType=NONE&map=67108864'],'Original Normal Undo identity/context differs')
            payload=json.loads(values[3]);need(payload['xid']==observation['xid'] and payload['branchId']==observation['branch_id'] and payload['@class']=='org.apache.seata.rm.datasource.undo.BranchUndoLog' and len(payload['sqlUndoLogs'])==2 and payload['sqlUndoLogs'][0]=='java.util.ArrayList' and len(payload['sqlUndoLogs'][1])==1,'Original persisted Normal Undo root differs')
            log=payload['sqlUndoLogs'][1][0];need(log['@class']=='org.apache.seata.rm.datasource.undo.SQLUndoLog' and log['sqlType']=='INSERT' and log['tableName']=='undo_work','Original Undo operation differs')
            need(log['beforeImage']=={'@class':'org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords','rows':['java.util.ArrayList',[]],'tableName':'undo_work'},'Original INSERT before image differs')
            row_class='org.apache.seata.rm.datasource.sql.struct.Row';field_class='org.apache.seata.rm.datasource.sql.struct.Field'
            fields=[{'@class':field_class,'keyType':'PRIMARY_KEY','name':'id','type':4,'value':1},{'@class':field_class,'keyType':'NULL','name':'v','type':12,'value':'prepare_not_committed'}]
            need(log['afterImage']=={'@class':'org.apache.seata.rm.datasource.sql.struct.TableRecords','rows':['java.util.ArrayList',[{'@class':row_class,'fields':['java.util.ArrayList',fields]}]],'tableName':'undo_work'},'Original physical after image differs')

    need(observation['registered_requests']==1 and observation['business_rows_after_rollback']==observation['undo_rows_after_rollback']==0 and observation['control_rows_after_rollback']==1 and observation['business_COMMIT'] is False,'Independent final database facts differ')
    transactions=[]
    for c,ending in [(business['connection'],'ROLLBACK'),(ownership['connection'],'COMMIT')]:
        session=[e for e in events if e['connection']==c];local=[e for e in mutations if e['connection']==c]
        begin=[e for e in session if e['kind']=='begin_received'];forward=[e for e in session if e['kind']=='begin_forwarded'];ack=[e for e in session if e['kind']=='begin_reply'];delivered=[e for e in session if e['kind']=='begin_reply_forwarded'];typed=[e for e in session if e['kind']=='query' and e['sql']=='BEGIN']
        need(len(begin)==len(forward)==len(ack)==len(delivered)==len(typed)==1,'Unique BEGIN request/forward/ACK required');ok_packet(ack[0],True,0)
        need(typed[0]['qpc_ns']<begin[0]['qpc_ns']<forward[0]['qpc_ns']<ack[0]['qpc_ns']<delivered[0]['qpc_ns']<local[0]['qpc_ns'] and frame(typed[0],0)==frame(begin[0],0)==frame(forward[0],0)==b'\x03BEGIN' and ack[0]['wire_hex']==delivered[0]['wire_hex'],'Original BEGIN byte/order binding differs')
        need([e['sql'] for e in session if e['kind']=='query' and e['sql'] in ['BEGIN','COMMIT','ROLLBACK']]==['BEGIN',ending],'Unexpected split transaction or business COMMIT')
        delivered_dml=[]
        for e in local:
            replies=[v for v in session if v['kind']=='dml_reply' and v['sql']==e['sql']];copies=[v for v in session if v['kind']=='dml_reply_forwarded' and v['sql']==e['sql']]
            need(len(replies)==len(copies)==1 and e['qpc_ns']<replies[0]['qpc_ns']<copies[0]['qpc_ns'] and replies[0]['wire_hex']==copies[0]['wire_hex'],'DML original ACK not delivered exactly once');ok_packet(replies[0],True,0 if e['sql'] in [TEMP_CREATE,TEMP_DROP] else 1);delivered_dml+=copies
        prefix=ending.lower();received=[e for e in session if e['kind']==prefix+'_received'];forwarded=[e for e in session if e['kind']==prefix+'_forwarded'];reply=[e for e in session if e['kind']==prefix+'_reply'];copies=[e for e in session if e['kind']==prefix+'_reply_forwarded']
        need(len(received)==len(forwarded)==len(reply)==len(copies)==1,'Original ending request/ACK required')
        if ending=='ROLLBACK':need(frame(reply[0],1)==bytes.fromhex('00000002020200'),'Original ROLLBACK must carry exactly the two independently observed temporary-DDL warnings')
        else:ok_packet(reply[0],False,0)
        typed_end=[e for e in session if e['kind']=='query' and e['sql']==ending];need(len(typed_end)==1 and typed_end[0]['qpc_ns']<received[0]['qpc_ns'] and typed_end[0]['wire_hex']==received[0]['wire_hex'],'Ending marker not bound to original typed/raw command')
        need(max(e['qpc_ns'] for e in delivered_dml)<received[0]['qpc_ns']<forwarded[0]['qpc_ns']<reply[0]['qpc_ns']<copies[0]['qpc_ns'] and frame(received[0],0)==frame(forwarded[0],0)==b'\x03'+ending.encode() and reply[0]['wire_hex']==copies[0]['wire_hex'],'Original ending byte/order binding differs')
        for e in session:
            if e['qpc_ns']<begin[0]['qpc_ns']:continue
            if e['kind'] in ['query','prepare','execute']:
                sql=e['sql'];need(';' not in sql and '/*' not in sql and '--' not in sql,'SQL boundary escape');need(sql in TEMP+[business['sql'],ENROLL,NORMAL,'SHOW GRANTS'] or sql.startswith('SELECT ') or (e['kind']=='query' and sql in ['BEGIN',ending]),'Unclassified SQL inside original transaction')
                if e['kind']=='query':need(sql in ['BEGIN',ending],'Unexpected unprepared mutation or read inside transaction')
            if e['kind']=='client_command':need(e['opcode'] in [1,3,22,23,25],'Unexpected protocol command inside transaction')
        transactions.append(dict(connection=c,mysql_connection_id=physical[c],ending=ending,ending_ack_qpc_ns=copies[0]['qpc_ns']))
    need(sum(e['kind']=='begin_received' for e in events)==2 and transactions[1]['ending_ack_qpc_ns']<undo['qpc_ns'],'Enrollment must durably acknowledge before original Undo write')
    return dict(verified=True,transactions=transactions,business_and_Undo_same_original_transaction=True,ownership_committed_separately=True,business_COMMIT=False,normal_Undo_written_then_rolled_back=True,temporary_key_staging_inside_original_transaction=True)

def negative_controls(wire,observation,warning_probe):
    cases=[]
    def reject(name,w,o):
        try:validate(w,o,warning_probe)
        except (ValueError,KeyError,IndexError):cases.append(name);return
        raise ValueError('Preparation raw mutation accepted: '+name)
    for kind in ['client_command','prepare_ok','begin_forwarded','begin_reply','begin_reply_forwarded','dml_reply','dml_reply_forwarded','rollback_reply','rollback_reply_forwarded','commit_reply']:
        changed=copy.deepcopy(wire);changed['events'].pop(next(i for i,e in enumerate(changed['events']) if e['kind']==kind));reject('missing-'+kind,changed,observation)
    for key,value in [('branch_id',42),('resource_id','other'),('control_rows_after_rollback',0),('business_rows_after_rollback',1),('business_COMMIT',True)]:
        changed=copy.deepcopy(observation);changed[key]=value;reject('wrong-'+key,wire,changed)
    changed=copy.deepcopy(wire);next(e for e in changed['events'] if e['kind']=='begin_forwarded')['wire_hex']='0700000003434f4d4d4954';reject('BEGIN-forwarded-as-COMMIT',changed,observation)
    changed=copy.deepcopy(wire);row=next(e for e in changed['events'] if e['kind']=='rollback_reply');raw=bytearray.fromhex(row['wire_hex']);raw[-2:]=b'\x01\x00';row['wire_hex']=raw.hex();reject('missing-second-temporary-DDL-warning',changed,observation)
    for name,probe in [('missing-independent-warning-probe',''),('nontransactional-business-warning',warning_probe.replace('1751','1196'))]:
        try:validate(wire,observation,probe)
        except (ValueError,KeyError,IndexError):cases.append(name);continue
        raise ValueError('Warning probe mutation accepted: '+name)
    return dict(all_rejected=True,count=len(cases),cases=cases)

def bound_values(events,event):
    replies=[e for e in events if e['kind']=='prepare_ok' and e['connection']==event['connection'] and int.from_bytes(frame(e,1)[1:5],'little')==event['statement'] and e['qpc_ns']<event['qpc_ns']]
    need(len(replies)==1,'Unique original prepared parameter metadata required')
    count=int.from_bytes(frame(replies[0],1)[7:9],'little')
    return native.parameters(event,count)

def validate_enrollment_fault(wire,observation):
    mode=wire['mode'];need(mode in ['block_commit','drop_ok','delay_ok'] and observation['mode']==mode and wire['claimed'] is True,'Enrollment fault role differs')
    need(observation['first']=='PreparationFailedWithKnownReceipt' and observation['before']==([0,0,0] if mode=='block_commit' else [1,0,0]) and observation['after']==[1,2,1] and observation['origin_after_callback']==(2 if mode=='block_commit' else 1),'Independent enrollment/callback facts differ')
    need(observation['original_register_requests']==1 and observation['business_rows']==observation['Normal_Undo_rows']==0 and observation['guard_rows']==1 and observation['business_COMMIT'] is False and observation['callback']=='PhaseTwoRollbacked','Replay or final resource facts differ')
    events,executes,physical=base.transcript(wire);xid=observation['xid'].encode();branch=observation['branch_id'];resource=observation['resource_id'].encode()
    callback_insert='INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,application_data,lock_key,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(2,?,?,?,?,NULL,2,?,?,0,now(6),now(6))'
    bind='UPDATE seata_rs_at_branch_v1 SET phase=?,modified_at=now(6) WHERE xid=? AND branch_id=? AND phase=0 AND terminal=0'
    guard='INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,1,now(6),now(6))'
    terminal=base.TERMINAL
    expected={ENROLL:[xid,branch,resource,b'prepare-fixture-app',b'undo_work:1',1],callback_insert:[xid,branch,resource,b'prepare-fixture-app',1,2],bind:[2,xid,branch],guard:[branch,xid,b'serializer=jackson&compressorType=NONE',b'{}'],terminal:[xid,branch]}
    mutations=[e for e in executes if e['sql'].startswith(('INSERT ','UPDATE ','DELETE ')) or e['sql'] in TEMP]
    want=TEMP+["INSERT INTO undo_work(id,v) VALUES(1,'prepared_fixture')",ENROLL,callback_insert if mode=='block_commit' else bind,guard,terminal]
    need([e['sql'] for e in mutations]==want,'Original preparation/callback mutation order differs')
    for e in mutations:
        if e['sql'] in expected:need(bound_values(events,e)==expected[e['sql']],'Original known receipt or callback identity/profile differs')
    local=mutations[:4];enrollment=mutations[4];handoff=mutations[5];effects=mutations[6:]
    need(len({e['connection'] for e in local})==1 and effects[0]['connection']==effects[1]['connection'] and len({local[0]['connection'],enrollment['connection'],handoff['connection'],effects[0]['connection']})==4,'Original physical transaction roles differ')
    fault=[e for e in events if e['kind']=='commit_received' and e['fault']];need(len(fault)==1 and fault[0]['connection']==enrollment['connection'],'Fault must target original known enrollment COMMIT')
    validate_enrollment_sessions(events,local,enrollment,handoff,effects,mode)
    confirmed=[e for e in events if e['connection']==handoff['connection'] and e['kind']=='commit_reply_forwarded'];effect_begin=[e for e in events if e['connection']==effects[0]['connection'] and e['kind']=='begin_received']
    need(len(confirmed)==len(effect_begin)==1 and confirmed[0]['qpc_ns']<effect_begin[0]['qpc_ns'],'Callback ownership must acknowledge before starting guarded effect transaction')
    return dict(verified=True,mode=mode,original_branch_id=branch,known_receipt_preserved=True,Normal_Undo_not_flushed=True,callback_completed=True,origin_after_callback=observation['origin_after_callback'],physical_connections=len(physical),business_COMMIT=False)

def validate_enrollment_sessions(events,local,enrollment,handoff,effects,mode):
    need(not any(e['kind']=='query' and e['sql'].startswith(('INSERT ','UPDATE ','DELETE ')) for e in events),'Unexpected original COM_QUERY mutation')
    selected=local+[enrollment,handoff]+effects
    for c in {e['connection'] for e in selected}:
        session=[e for e in events if e['connection']==c];mutations=[e for e in selected if e['connection']==c]
        beginnings=[e for e in session if e['kind']=='query' and e['sql']=='BEGIN'];received=[e for e in session if e['kind']=='begin_received'];forwarded=[e for e in session if e['kind']=='begin_forwarded'];acks=[e for e in session if e['kind']=='begin_reply'];copies=[e for e in session if e['kind']=='begin_reply_forwarded']
        need(len(beginnings)==len(received)==len(forwarded)==len(acks)==len(copies)==1,'Unique original BEGIN required');ok_packet(acks[0],True,0)
        need(beginnings[0]['qpc_ns']<received[0]['qpc_ns']<forwarded[0]['qpc_ns']<acks[0]['qpc_ns']<copies[0]['qpc_ns']<mutations[0]['qpc_ns'] and frame(beginnings[0],0)==frame(received[0],0)==frame(forwarded[0],0)==b'\x03BEGIN' and acks[0]['wire_hex']==copies[0]['wire_hex'],'BEGIN identity/order differs')
        delivered=[]
        for e in mutations:
            reply=[r for r in session if r['kind']=='dml_reply' and r['sql']==e['sql']];sent=[r for r in session if r['kind']=='dml_reply_forwarded' and r['sql']==e['sql']]
            need(len(reply)==len(sent)==1 and e['qpc_ns']<reply[0]['qpc_ns']<sent[0]['qpc_ns'] and reply[0]['wire_hex']==sent[0]['wire_hex'],'Original mutation reply not delivered once');ok_packet(reply[0],True,0 if e['sql'] in [TEMP_CREATE,TEMP_DROP] else 1);delivered+=sent
        is_local=c==local[0]['connection'];is_fault=c==enrollment['connection'];boundaries=[e['sql'] for e in session if e['kind']=='query' and e['sql'] in ['BEGIN','COMMIT','ROLLBACK']]
        need(boundaries in ([['BEGIN'],['BEGIN','ROLLBACK']] if is_local else [['BEGIN','COMMIT'],['BEGIN','COMMIT','ROLLBACK']] if is_fault else [['BEGIN','COMMIT']]),'Unexpected split transaction or local COMMIT')
        allowed={e['sql'] for e in mutations}
        for e in session:
            if e['qpc_ns']<received[0]['qpc_ns']:continue
            if e['kind'] in ['query','prepare','execute']:
                sql=e['sql'];need(';' not in sql and '/*' not in sql and '--' not in sql,'SQL boundary escape');need(sql in allowed or sql=='SHOW GRANTS' or sql.startswith('SELECT ') or e['kind']=='query' and sql in ['BEGIN','COMMIT','ROLLBACK'],'Unclassified SQL inside control transaction')
                if e['kind']=='query':need(sql in ['BEGIN','ROLLBACK'] if is_local else sql in ['BEGIN','COMMIT','ROLLBACK'],'Unexpected unprepared SQL')
            if e['kind']=='client_command':need(e['opcode'] in [1,3,22,23,25],'Unexpected native command')
        if is_local:
            need(not any(e['kind'].startswith('commit_') for e in session),'Local business must not COMMIT');continue
        verify_control_commit(session,delivered,is_fault,mode)

def verify_control_commit(session,delivered,is_fault,mode):
    typed=[e for e in session if e['kind']=='query' and e['sql']=='COMMIT'];received=[e for e in session if e['kind']=='commit_received'];forward=[e for e in session if e['kind']=='commit_forwarded'];acks=[e for e in session if e['kind']=='commit_reply'];copies=[e for e in session if e['kind']=='commit_reply_forwarded']
    need(len(typed)==len(received)==1 and max(e['qpc_ns'] for e in delivered)<typed[0]['qpc_ns']<received[0]['qpc_ns'] and received[0]['fault']==is_fault and typed[0]['wire_hex']==received[0]['wire_hex'],'COMMIT not bound to original post-mutation request')
    if is_fault and mode=='block_commit':
        need(not forward and not acks and not copies and sum(e['kind']=='commit_blocked' for e in session)==1,'Blocked enrollment COMMIT reached server');return
    need(len(forward)==len(acks)==1 and received[0]['qpc_ns']<forward[0]['qpc_ns']<acks[0]['qpc_ns'] and received[0]['wire_hex']==forward[0]['wire_hex'],'Original COMMIT/ACK provenance differs');ok_packet(acks[0],False,0)
    need(len(copies)==(0 if is_fault else 1),'COMMIT ACK delivery differs')
    if not is_fault:
        need(acks[0]['qpc_ns']<copies[0]['qpc_ns'] and acks[0]['wire_hex']==copies[0]['wire_hex'],'Delivered ACK bytes differ');return
    tag='commit_reply_held' if mode=='delay_ok' else 'commit_reply_dropped';marker=[e for e in session if e['kind']==tag];need(len(marker)==1 and acks[0]['qpc_ns']<marker[0]['qpc_ns'],'Original ACK was not uniquely lost/held')
    if mode=='delay_ok':
        late=[e for e in session if e['kind'] in ['late_reply_sent','late_reply_peer_closed']];need(len(late)==1 and late[0]['qpc_ns']-marker[0]['qpc_ns']>=2_100_000_000,'Original enrollment ACK did not actually exceed deadline')

def enrollment_negative_controls(wire,observation):
    cases=[]
    def reject(name,w,o):
        try:validate_enrollment_fault(w,o)
        except (ValueError,KeyError,IndexError):cases.append(name);return
        raise ValueError('Enrollment mutation accepted: '+name)
    for kind in ['client_command','prepare_ok','begin_received','begin_forwarded','begin_reply','begin_reply_forwarded','dml_reply','dml_reply_forwarded','commit_received']:
        changed=copy.deepcopy(wire);changed['events'].pop(next(i for i,e in enumerate(changed['events']) if e['kind']==kind));reject('missing-'+kind,changed,observation)
    for key,value in [('branch_id',1),('resource_id','other'),('original_register_requests',2),('business_COMMIT',True),('Normal_Undo_rows',1),('origin_after_callback',0)]:
        changed=copy.deepcopy(observation);changed[key]=value;reject('wrong-'+key,wire,changed)
    changed=copy.deepcopy(wire);next(e for e in changed['events'] if e['kind']=='begin_forwarded')['wire_hex']='0700000003434f4d4d4954';reject('BEGIN-forwarded-as-COMMIT',changed,observation)
    changed=copy.deepcopy(wire);handoff=next(e for e in changed['events'] if e['kind']=='execute' and (e['sql'].startswith('UPDATE seata_rs_at_branch_v1 SET phase=') or 'VALUES(2,?,?,?,?,NULL,2,?,?,0,now(6),now(6))' in e['sql']));guard=next(e for e in changed['events'] if e['kind']=='execute' and e['sql'].startswith('INSERT INTO undo_log ') and ',1,now(6),now(6))' in e['sql'])
    for offset,kind in [(1,'commit_reply'),(2,'commit_reply_forwarded')]:next(e for e in changed['events'] if e['connection']==handoff['connection'] and e['kind']==kind)['qpc_ns']=guard['qpc_ns']+offset
    changed['events'].sort(key=lambda e:e['qpc_ns']);reject('effect-before-callback-ownership-confirmation',changed,observation)
    if wire['mode']=='delay_ok':
        changed=copy.deepcopy(wire);changed['events']=[e for e in changed['events'] if e['kind'] not in ['late_reply_sent','late_reply_peer_closed']];reject('missing-actual-late-ACK',changed,observation)
    return dict(all_rejected=True,count=len(cases),cases=cases)
