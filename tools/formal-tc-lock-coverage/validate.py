"""Independent raw frames, actual DB rows and native key evidence; observation PASS never means TC safety PASS."""
import importlib.util
import json
from pathlib import Path
import struct
import base64

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('coverage_native_validator',HERE.parent/'formal-rm-java/validate.py')
wire=importlib.util.module_from_spec(spec);spec.loader.exec_module(wire)
need=wire.need

# These are expected original observations, including coverage defects; never acceptance criteria for production safety.
OBSERVED={
    'normal':('t:1','t:1',[('t','1')],False),
    'empty':('t:','t:',[],True),
    'blank':('t: ','t: ',[],True),
    'unicode-blank':('t:\u2003','t:\u2003',[],True),
    'partial-empty':('t:,1','t:',[('t','1')],True),
    'semicolon':('t:x;broken,z','t:z',[('t','x')],True),
    'binary-same':('t_binary:[0, -1]','t_binary:[0, -1]',[('t_binary','[0'),('t_binary',' -1]')],False),
    'binary-distinct':('t_binary:[0, -1]','t_binary:[0, -2]',[('t_binary','[0'),('t_binary',' -1]')],False),
}

def frames(rows,role):
    summary=[r for r in rows if r['event']=='summary']
    need(len(summary)==1 and summary[0]['succeeded'] is True and summary[0]['faults']==0,'Missing successful passthrough observer')
    need(not any(r['event']=='error' for r in rows),'Observer error')
    need(all(type(r.get('index')) is int and r['index']==index for index,r in enumerate(rows)),'Observer rows reordered or duplicated')
    need(all(type(r.get('monotonic_ns')) is int and r['monotonic_ns']>0 for r in rows) and all(a['monotonic_ns']<=b['monotonic_ns'] for a,b in zip(rows,rows[1:])),'Observer QPC sequence differs')
    pending={};callbacks={};complete=[];ids=set();registered=set()
    for row in rows:
        if row['event']!='frame':continue
        event=wire.envelope(row);need(row['role']==role and row['delivery_count']==1 and row['action']=='forward','Unexpected observer role/fault')
        body=event['body'];typ=row['request_type'];kind=row['kind'];key=(row['connection'],row['request_id'])
        if typ in [3,4]:continue
        need(not event['head_bytes'],'Unexpected native fixture header extensions')
        if row['direction']=='c2s' and typ==0:
            if kind not in [101,103]:need(row['connection'] in registered,'Business before native registration identified')
            need(key not in ids,'Original request ID reused');ids.add(key);pending[key]=event
        elif row['direction']=='s2c' and typ==1:
            need(key in pending,'Response lacks original physical request');original=pending.pop(key)
            need(kind==original['kind']+1,'Response message family differs')
            if kind in [102,104]:
                reader=wire.Reader(original['body']);reader.take(2)
                fields=[reader.string() for _ in range(4)]
                need(fields==['1.1.0','phase0-auth','default_tx_group','phase0-local-only'],'Native registration role/application identity differs')
                if role=='RM':need(kind==104 and reader.string(True)=='mysql://auth-a','Native RM resource registration differs')
                else:need(kind==102,'Native TM registration family differs')
                reader.done();reader=wire.Reader(body);reader.take(2);need(reader.number('>B')==1,'Native registration not identified');reader.string();reader.done();registered.add(row['connection'])
            complete.append((original,event))
        elif role=='RM' and row['direction']=='s2c' and typ==0:
            need(kind==5 and key not in callbacks and row['connection'] in registered,'Unexpected or repeated original callback');callbacks[key]=event
        elif role=='RM' and row['direction']=='c2s' and typ==1:
            need(key in callbacks and kind==6,'Reply lacks original callback');original=callbacks.pop(key)
            ident=wire.callback(original['body']);reader=wire.Reader(body);reader.take(2)
            need(reader.take(2)==b'\x01\x00','Callback result failed')
            xid=reader.string();branch=reader.number('>q');status=reader.number('>B');reader.done()
            need((xid,branch,status)==(ident['xid'],ident['branch_id'],8),'Callback identity/status differs')
            complete.append((original,event))
        else:raise ValueError('Unexpected original observer direction/type')
    need(not pending and not callbacks,'Original requests/callbacks unanswered')
    return complete

def response(event):
    reader=wire.Reader(event['body']);reader.take(2);code=reader.number('>B');need(code in [0,1],'Invalid response result')
    message=reader.string() if code==0 else None
    error=reader.number('>B')
    return reader,code,error,message

def native_key(log,name,role):
    def sequence(value):
        need(isinstance(value,list) and len(value)==2 and value[0]=='java.util.ArrayList' and isinstance(value[1],list),'Native Java typed-list shape differs');return value[1]
    prefix='org.apache.seata.rm.datasource.'
    need(log['@class']==prefix+'undo.BranchUndoLog' and log['branchId']==42 and log['xid']=='coverage-provenance:xid','Native prepared identity differs')
    entries=sequence(log['sqlUndoLogs']);need(len(entries)==1 and entries[0]['@class']==prefix+'undo.SQLUndoLog' and entries[0]['sqlType']=='INSERT','Native prepared INSERT log differs')
    entry=entries[0];image=entry['afterImage'];table='t_binary' if name.startswith('binary') else 't'
    need(entry['tableName']==image['tableName']==table and image['@class']==prefix+'sql.struct.TableRecords','Native actual table differs')
    before=entry['beforeImage'];need(before['tableName']==table and before['@class']==prefix+'sql.struct.TableRecords$EmptyTableRecords' and sequence(before['rows'])==[],'Native INSERT before image differs')
    values=[];formatted=[]
    for row in sequence(image['rows']):
        fields=sequence(row['fields']);need(row['@class']==prefix+'sql.struct.Row' and len(fields)==1,'Native actual row/field shape differs')
        field=fields[0];need(field['@class']==prefix+'sql.struct.Field' and field['name']=='id' and field['keyType']=='PRIMARY_KEY','Native primary field differs')
        value=field['value']
        if table=='t_binary':
            need(field['type']==-3 and isinstance(value,list) and len(value)==2 and value[0]=='[B','Native binary JDBC class differs')
            data=base64.b64decode(value[1],validate=True);values.append(data.hex());formatted.append('['+', '.join(str(x if x<128 else x-256) for x in data)+']')
        else:
            need(field['type']==12 and type(value) is str,'Native text JDBC value differs');values.append(value);formatted.append(value)
    expected={'normal':['1'],'empty':[''],'blank':[' '],'unicode-blank':['\u2003'],
        'partial-empty':['','1'] if role=='owner' else [''],'semicolon':['x;broken','z'] if role=='owner' else ['z'],
        'binary-same':['00ff'],'binary-distinct':['00ff'] if role=='owner' else ['00fe']}[name]
    need(values==expected,'Native actual primary values/order differ')
    return table+':'+','.join(formatted)

def validate(cases,checkpoints,rm_rows,tm_rows,nonce,client_result,native_logs):
    need([r['case'] for r in cases]==list(OBSERVED),'Actual native cases differ')
    need(len(checkpoints)==25,'DB checkpoint count differs')
    provenance=checkpoints[0];need(provenance['stage']=='provenance' and provenance['data']['cases']==cases,'Native provenance checkpoint differs')
    need(all(v==0 for v in provenance['business_counts'].values()),'Prepared business rows/Undo remained after rollback')
    rm=frames(rm_rows,'RM');tm=frames(tm_rows,'TM')
    branch_requests=[];locks=[];callbacks=[];begins=[];ends=[];statuses=[]
    app=json.dumps({'tcLockCoverage':nonce},separators=(',',':'))
    for request,reply in rm:
        if request['kind']==103:continue
        if request['kind']==5:
            ident=wire.callback(request['body']);need(ident['data']==app and ident['resource']=='mysql://auth-a' and ident['branch_type']==0,'Callback application/resource differs')
            callbacks.append(dict(ident,request_ns=request['monotonic_ns'],reply_ns=reply['monotonic_ns']));continue
        reader=wire.Reader(request['body']);kind=reader.number('>H')
        need(kind in [11,21],'Unexpected RM request')
        ident=dict(xid=reader.string(),branch_type=reader.number('>B'),resource=reader.string(),key=reader.string(True),data=reader.string(True));reader.done()
        need(ident['data']==app and ident['resource']=='mysql://auth-a' and ident['branch_type']==0,'Original RM application/resource differs')
        result,code,error,message=response(reply)
        if kind==11:
            branch=result.number('>q');result.done()
            need((code==1 and error==0 and branch>0) or (code==0 and error in [2,20] and branch==0 and bool(message)),'Registration prefix/result differs')
            branch_requests.append(dict(ident,registered=code==1,branch_id=branch,error=error,error_message=message,request_ns=request['monotonic_ns'],reply_ns=reply['monotonic_ns']))
        else:
            lockable=result.number('>H');result.done();need(code==1 and error==0 and lockable in [0,1],'Lock query response differs')
            locks.append(dict(ident,lockable=bool(lockable),request_ns=request['monotonic_ns'],reply_ns=reply['monotonic_ns']))
    for request,reply in tm:
        kind=request['kind']
        if kind==101:continue
        data=wire.Reader(request['body']);data.take(2);result,code,error,_=response(reply)
        need(code==1 and error==0,'Original TM result was not successful')
        if kind==1:
            timeout=data.number('>i');name=data.string();data.done();xid=result.string();extra=result.string();result.done()
            need(timeout==180000 and extra is None and bool(xid),'Begin timeout/XID differs');begins.append(dict(xid=xid,name=name,request_ns=request['monotonic_ns'],reply_ns=reply['monotonic_ns']))
        elif kind in [9,15]:
            xid=data.string();need(data.string() is None,'Original TM optional extraData differs');data.done();status=result.number('>B');result.done()
            if kind==9:need(status==11,'Original rollback did not explicitly return Rollbacked')
            (ends if kind==9 else statuses).append(dict(xid=xid,status=status,request_ns=request['monotonic_ns'],reply_ns=reply['monotonic_ns']))
        else:raise ValueError('Unexpected original TM operation')
    need(len(branch_requests)==16 and len(locks)==8 and len(begins)==16 and len(ends)==16,'Original mutating/query request counts differ')
    need(len({r['xid'] for r in begins})==16,'Begin identities repeated')
    successful=[];observations=[]
    for index,case in enumerate(cases):
        name=case['case'];owner_key,contender_key,owner_rows,lockable=OBSERVED[name]
        need((case['owner_key'],case['contender_key'])==(owner_key,contender_key),'Native generated key differs')
        stages=checkpoints[1+index*3:4+index*3];need([r['stage'] for r in stages]==['owner','contender','released'] and all(r['index']==index for r in stages),'DB checkpoint ordering differs')
        data=stages[1]['data'];owner=data['owner_xid'];contender=data['contender_xid']
        need(owner!=contender and all(r['data']['owner_xid']==owner and r['data']['contender_xid']==contender for r in stages),'Checkpoint original identities differ')
        for role,xid,key in [('owner',owner,owner_key),('contender',contender,contender_key)]:
            matches=[r for r in branch_requests if r['xid']==xid];need(len(matches)==1 and matches[0]['key']==key,'Original registration/key differs or repeated')
            item=matches[0];expected=role=='owner' or lockable
            need(item['registered']==expected and data[role]['registered']==expected,'Client registration differs from actual original result')
            if expected:
                need(data[role]['branch_id']==item['branch_id'],'Client BranchId differs');successful.append((xid,item['branch_id']))
            names=[r['name'] for r in begins if r['xid']==xid]
            need(names==[f'lock-{nonce}-{name}-{role}'],'Actual Begin name/identity differs')
            begin=next(r for r in begins if r['xid']==xid);need(begin['reply_ns']<item['request_ns'],'Registration precedes original Begin reply')
            end=[r for r in ends if r['xid']==xid];need(len(end)==1,'Global end repeated or missing');end=end[0]
            need(stages[2]['data'][role+'_end']==dict(xid=xid,status=end['status']),'Client original end differs from actual response')
            final=[r for r in statuses if r['xid']==xid and r['status']==15 and r['request_ns']>end['reply_ns']]
            need(final and final[-1]['reply_ns']<stages[2]['observer_time_ns'],'Final cleanup query missing or precedes original end')
            need(item['reply_ns']<stages[1 if role=='contender' else 0]['observer_time_ns'] and end['request_ns']>stages[1]['observer_completed_ns'],'DB checkpoint/end ordering differs')
            for callback in [r for r in callbacks if r['xid']==xid]:need(end['request_ns']<callback['request_ns']<callback['reply_ns']<end['reply_ns'],'Callback did not belong to the observed original end interval')
        need([r['lockable'] for r in locks if r['xid']==contender and r['key']==contender_key]==[lockable] and data['contender_lockable']==lockable,'Independent lock result differs')
        query=next(r for r in locks if r['xid']==contender);registration=next(r for r in branch_requests if r['xid']==contender)
        need(stages[0]['observer_time_ns']<=stages[0]['observer_completed_ns']<query['request_ns']<query['reply_ns']<registration['request_ns']<registration['reply_ns']<stages[1]['observer_time_ns'],'Owner observation/contender conflict-query/registration ordering differs')
        for stage in stages[:2]:
            expected_rows=[(owner,table,pk) for table,pk in owner_rows]
            expected_branches=[(owner,data['owner']['branch_id'])]
            if stage['stage']=='contender' and lockable:
                expected_branches.append((contender,data['contender']['branch_id']))
                if name=='semicolon':expected_rows.append((contender,'t','z'))
            actual_rows=[(r['xid'],r['table'],r['pk']) for r in stage['locks']]
            need(sorted(actual_rows)==sorted(expected_rows),'Actual DB lock rows differ')
            need(sorted((r['xid'],r['branch_id']) for r in stage['branches'])==sorted(expected_branches),'Actual DB branch rows differ')
            need(sorted(r['xid'] for r in stage['globals'])==sorted([owner,contender]),'Actual DB global rows differ')
            need(all(r['status']==1 for r in stage['globals']) and all(r['resource']=='mysql://auth-a' for r in stage['branches']),'Original DB global status/branch resource differs')
            for row in stage['locks']:
                need(row['resource']=='mysql://auth-a' and (row['xid'],row['branch_id']) in expected_branches,'DB lock identity/resource differs')
                need(row['row_key']=='mysql://auth-a^^^'+row['table']+'^^^'+row['pk'],'DB physical lock identity differs')
        need(not stages[2]['locks'] and not stages[2]['branches'] and not stages[2]['globals'],'Original TC storage was not released')
        # Independently bind the real MySQL native typed images to the lock keys supplied to this network run.
        for role in ['owner','contender']:
            need(native_key(native_logs[f'native-{index}-{role}.json'],name,role)==case[role+'_key'],'Actual native primary values do not generate the original registered key')
        observations.append(dict(case=name,owner_lock_rows=len(owner_rows),contender_lockable=lockable,contender_registered=lockable))
    need(sorted((r['xid'],r['branch_id']) for r in callbacks)==sorted(successful),'Original callback identities/counts differ')
    need(client_result['registered']==len(successful)==client_result['callback_apply_calls']==client_result['fixture_effects'] and client_result['business_commit'] is False,'Client fixture effect counts differ')
    return dict(observations=observations,actual_Begin_requests=len(begins),actual_global_end_requests=len(ends),actual_branch_register_requests=len(branch_requests),actual_lock_queries=len(locks),actual_callbacks=len(callbacks),definite_lock_conflicts=[r for r in branch_requests if not r['registered']],TC_safety_gate_passed=False,TC_registration_gate_passed=False,SQL_Undo_atomicity_verified=False,public_COMMIT_available=False)
