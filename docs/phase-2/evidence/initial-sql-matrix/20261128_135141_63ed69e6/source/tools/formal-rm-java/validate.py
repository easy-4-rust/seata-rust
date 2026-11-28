"""Independent raw-byte RM verifier; client claims cannot replace TC identity, actual copies, or journal state."""
import json
import re
import struct
import zlib


def need(condition,reason):
    if not condition:raise ValueError(reason)


class Reader:
    def __init__(self,data):self.data=data;self.at=0
    def take(self,n):
        need(n>=0 and self.at+n<=len(self.data),'Truncated bounded field')
        value=self.data[self.at:self.at+n];self.at+=n;return value
    def number(self,fmt):return struct.unpack(fmt,self.take(struct.calcsize(fmt)))[0]
    def string(self,wide=False):
        length=self.number('>i' if wide else '>h');need(length>=0,'Negative native string length')
        return self.take(length).decode('utf-8') if length else None
    def done(self):need(self.at==len(self.data),'Trailing native payload bytes')


def callback(payload):
    reader=Reader(payload);kind=reader.number('>H');need(kind in [3,5],'Not a branch callback')
    result=dict(kind=kind,xid=reader.string(),branch_id=reader.number('>q'),branch_type=reader.number('>B'),resource=reader.string(),data=reader.string(True));reader.done();return result


def envelope(row):
    try:raw=bytes.fromhex(row['frame_hex'])
    except (KeyError,ValueError,TypeError) as error:raise ValueError('Missing raw frame') from error
    need(raw.hex()==row['frame_hex'] and len(raw)>=16,'Noncanonical raw frame')
    full,head=struct.unpack('>IH',raw[3:9]);typ=raw[9];wire=struct.unpack('>i',raw[12:16])[0]
    need(raw[:3]==b'\xda\xda\x01' and len(raw)==full and 16<=head<=full<=65536,'Invalid V1 envelope')
    need(typ in [0,1,2,3,4] and raw[10:12]==b'\x01\x00','Unexpected native codec/type')
    body=raw[head:];kind=struct.unpack('>H',body[:2])[0] if len(body)>=2 else None
    need((not body and typ in [3,4]) or (kind is not None and typ in [0,1,2]),'Invalid heartbeat/body shape')
    for key,expected in [('request_type',typ),('request_id',wire),('kind',kind)]:
        need((type(row.get(key)) is int if expected is not None else row.get(key) is None) and row.get(key)==expected,'Metadata differs from raw native bytes')
    need(type(row.get('connection')) is int and row['connection']>0,'Invalid origin connection')
    need(type(row.get('delivery_count')) is int and row['delivery_count'] in [0,1,2],'Invalid actual delivery count')
    actions={'forward':1,'hold':0,'release_original':1,'duplicate_original':2,'drop_and_close':0}
    need(row.get('action') in actions and row['delivery_count']==actions[row['action']],'Action and actual delivery count disagree')
    need(row.get('direction') in ['c2s','s2c'],'Invalid direction')
    return dict(row,body=body,head_bytes=raw[16:head],native_kind=kind)


def journal_records(data,expected_payload,*,expected_states=(1,3)):
    need(data[:8]==b'SRMJNL01','Journal version differs');at=8;records=[]
    while at<len(data):
        need(at+4<=len(data),'Truncated journal record prefix');length=struct.unpack('>I',data[at:at+4])[0];at+=4
        need(13<=length<=65532 and at+length<=len(data),'Invalid journal record budget/length');raw=data[at:at+length];at+=length
        need(zlib.crc32(raw[:-4])==struct.unpack('>I',raw[-4:])[0],'Journal checksum differs')
        sequence,state=struct.unpack('>QB',raw[:9]);need(sequence==len(records)+1,'Journal sequence differs')
        need(raw[9:-4]==expected_payload,'Persisted branch identity differs from actual original TC callback')
        records.append(dict(sequence=sequence,state=state))
    need(tuple(row['state'] for row in records)==expected_states,'Unexpected durable journal sequence or duplicate writes')
    return records


def validate(case,text,rows,nonce,journal):
    need(case in [f'{mode}-{op}' for mode in ['basic','duplicate','reconnect'] for op in ['commit','rollback']],'Unexpected scenario')
    mode,operation=case.split('-');kind=3 if operation=='commit' else 5;terminal=5 if operation=='commit' else 8
    need(type(nonce) is str and re.fullmatch('[a-f0-9]{32}',nonce) is not None,'Invalid original nonce')
    lines=[line for line in text.splitlines() if line.startswith('PASS ')]
    pattern=r'PASS RM_NATIVE mode=(commit|rollback) xid=(\S+) branch_id=(\d+) contender=(\S+) nonce=([a-f0-9]{32}) apply_calls=(\d+) effects=(\d+) recover_calls=(\d+) journal_reopened=true lock_before=false lock_after=true'
    match=re.fullmatch(pattern,lines[0]) if len(lines)==1 else None;need(match is not None,'Missing unique full client result')
    op,xid,issued,contender,reported_nonce,applied,effects,recovered=match.groups();issued,applied,effects,recovered=map(int,[issued,applied,effects,recovered])
    need(op==operation and reported_nonce==nonce and issued>0 and xid!=contender and applied==1 and effects==1 and recovered==0,'Client identity/counters differ')
    ends=re.findall(r'^END mode=(commit|rollback) xid=(\S+) status=(\S+) unknown=(true|false)$',text,re.M)
    need(len(ends)==1 and ends[0][:2]==(operation,xid),'Missing original one-operation end observation')
    status,unknown=ends[0][2:]
    need((unknown=='true' and status=='unknown') or (unknown=='false' and status.isdecimal() and int(status) in ([2,3,8,9,15] if operation=='commit' else [4,5,6,7,11,13,15])),'Invalid original end result')
    need(all(type(row.get('index')) is int and row['index']==index for index,row in enumerate(rows)),'Missing, reordered or duplicated observer rows')
    summaries=[row for row in rows if row.get('event')=='summary'];need(len(summaries)==1,'Missing unique proxy summary');summary=summaries[0]
    need(summary.get('succeeded') is True and summary.get('mode')==mode and summary.get('errors')==[],'Proxy run did not complete scenario')
    for key in ['connections','faults','released','closed']:need(type(summary.get(key)) is int,'Summary counter type differs')
    need(not any(row.get('event')=='error' for row in rows),'Proxy recorded I/O/validation errors')
    app=json.dumps({'formalRm':nonce},separators=(',',':'))
    frames=[envelope(row) for row in rows if row.get('event')=='frame']
    pending={};used_ids=set();registered=set();requests=[];results=[];callbacks={};replies={}
    close_indices={row['connection']:row['index'] for row in rows if row.get('event')=='fault_close'}
    for frame in frames:
        row=frame;data=row['body'];reader=Reader(data);message=reader.number('>H') if data else None;key=(row['connection'],row['request_id'])
        if message is None:continue
        if message in [11,12,13,14,21,22] and row['connection'] in close_indices:
            need(row['index']<close_indices[row['connection']],'Business frame returned to a closed old session')
        if row['direction']=='c2s' and row['request_type']==0:
            need(message in [103,11,13,21] and key not in used_ids,'Unsupported/reused RM request ID')
            used_ids.add(key)
            need(row['action']=='forward' and row['delivery_count']==1,'RM outbound request was transformed/replayed')
            if message==103:
                fields=[reader.string(),reader.string(),reader.string(),reader.string(),reader.string(True)]
                need(fields==['1.1.0','phase0-auth','default_tx_group','phase0-local-only','mysql://auth-a'],'Native RM role/resource identity differs')
                value=dict(kind=message)
            else:
                need(row['connection'] in registered,'Business sent before this generation resource registration was accepted')
                if message==13:
                    value=dict(kind=message,xid=reader.string(),branch_id=reader.number('>q'),status=reader.number('>B'),resource=reader.string(),data=reader.string(True),branch_type=reader.number('>B'))
                    need(value==dict(kind=13,xid=xid,branch_id=issued,status=2,resource=None,data=app,branch_type=0),'BranchReport changed original identity, optionality or data')
                else:
                    value=dict(kind=message,xid=reader.string(),branch_type=reader.number('>B'),resource=reader.string(),lock=reader.string(True),data=reader.string(True))
                    need(value['resource']=='mysql://auth-a' and value['branch_type']==0 and value['lock']=='orders:1' and value['data']==app,'Original registered resource/lock/nonce differs')
                    if message==11:need(value['xid']==xid,'BranchRegister XID differs')
                    else:need(value['xid'] in [xid,contender],'Unexpected lock query XID')
            reader.done();value.update(index=row['index'],key=key);pending[key]=value;requests.append(value)
        elif row['direction']=='s2c' and row['request_type']==1:
            need(key in pending,'Response lacks unique original RM request');original=pending.pop(key)
            need(message==original['kind']+1 and row['action']=='forward' and row['delivery_count']==1,'RM response family/delivery differs')
            value=dict(kind=message,original=original,index=row['index'])
            if message==104:
                need(reader.number('>B')==1,'RM resource registration not identified');reader.string();registered.add(row['connection'])
            else:
                need(reader.take(2)==b'\x01\x00','Transaction Failed/exception prefix cannot claim success')
                if message==12:need(reader.number('>q')==issued,'Actual original TC-issued BranchId differs')
                if message==22:
                    boolean=reader.number('>h');need(boolean in [0,1],'Invalid raw lock boolean');value['lockable']=bool(boolean)
            reader.done();results.append(value)
        elif row['direction']=='s2c' and row['request_type']==0:
            role_requested=any(value['kind']==103 and value['key'][0]==row['connection'] for value in requests)
            need(message==kind and role_requested and key not in callbacks,'Unexpected/unregistered/reused callback origin')
            value=callback(data);need(value==dict(kind=kind,xid=xid,branch_id=issued,branch_type=0,resource='mysql://auth-a',data=app),'Callback differs from original registered branch and nonce')
            callbacks[key]=row
        elif row['direction']=='c2s' and row['request_type']==1:
            need(key in callbacks and message==kind+1,'Branch reply changed original connection/request ID/family');original=callbacks[key]
            need(row['connection'] in registered,'Callback replied before this generation resource registration was accepted')
            need(row['head_bytes']==original['head_bytes'],'Branch reply changed original headers')
            need(reader.take(2)==b'\x01\x00' and reader.string()==xid and reader.number('>q')==issued and reader.number('>B')==terminal,'Branch reply was not exact original confirmed phase result');reader.done()
            replies.setdefault(key,[]).append(row)
        else:raise ValueError('Unsupported RM envelope/direction')
    need(not pending,'A native RM request has no unique final response')
    need(len([r for r in requests if r['kind']==11])==1 and len([r for r in requests if r['kind']==13])==1,'Original BranchRegister/Report missing or replayed')
    locks=[r for r in results if r['kind']==22]
    need([(r['original']['xid'],r['lockable']) for r in locks]==[(xid,True),(contender,False),(contender,True)],'Lock competition/release claims differ from original raw requests/results')
    need(callbacks and all(key in replies for key in callbacks),'Missing actual callback replies')
    registration=[r for r in results if r['kind']==12][0]
    need(all(row['index']>registration['index'] for row in callbacks.values()),'Callback preceded original TC-issued branch identity')
    report=next(r for r in results if r['kind']==14)
    first_callback=min(row['index'] for row in callbacks.values())
    need(registration['index']<report['original']['index']<report['index']<locks[0]['original']['index']<locks[0]['index']<locks[1]['original']['index']<locks[1]['index']<first_callback,'Report/lock/callback causal order differs')
    delivered=[row['index'] for values in replies.values() for row in values if row['delivery_count']==1]
    need(delivered and max(delivered)<locks[2]['original']['index']<locks[2]['index'],'Final lock query preceded the last actually delivered successful RM acknowledgement')
    actions=[row for row in frames if row['action']!='forward']
    if mode=='basic':
        need(summary['faults']==summary['released']==summary['closed']==0 and summary['connections']==1 and not actions and len(callbacks)==1,'Basic scenario cannot contain fault/duplicate frames')
        need(all(len(replies[key])==1 and row['delivery_count']==1 for key,row in callbacks.items()),'Basic callback duplicated')
    elif mode=='duplicate':
        need(summary['faults']==1 and summary['released']==1 and summary['closed']==0 and summary['connections']==1 and len(callbacks)==1,'Duplicate scenario not uniquely observed')
        key,original=next(iter(callbacks.items()));answers=replies[key]
        need(original['action']=='duplicate_original' and original['delivery_count']==2 and len(answers)==2,'Original callback was not actually delivered twice and answered twice')
        need([r['action'] for r in answers]==['hold','release_original'] and [r['delivery_count'] for r in answers]==[0,1],'Two actual replies must release exactly one original response to TC')
        need(answers[0]['frame_hex']==answers[1]['frame_hex']==answers[1].get('released_frame_hex'),'Released reply changed held original bytes')
        need(len(actions)==3,'Unexpected additional duplicate/fault action')
    else:
        drops=[row for row in frames if row['action']=='drop_and_close'];closes=[row for row in rows if row.get('event')=='fault_close']
        need(summary['faults']==1 and summary['closed']==1 and summary['released']==0 and len(drops)==len(closes)==1 and len(callbacks)>=2,'Reconnect did not lose exactly one actual confirmed reply')
        dropped=drops[0];need(dropped['direction']=='c2s' and dropped['request_type']==1 and dropped['delivery_count']==0,'Wrong dropped frame')
        need(closes[0]['connection']==dropped['connection'] and closes[0]['index']>dropped['index'],'Original dropped connection not explicitly closed')
        later=[row for row in callbacks.values() if row['connection']!=dropped['connection'] and row['index']>closes[0]['index']]
        need(later and summary['connections']>=2 and len(registered)>=2,'No actual TC retry on a newly registered RM generation')
        need(all(row['action']=='forward' and row['delivery_count']==1 for row in callbacks.values()) and all(len(value)==1 for value in replies.values()) and len(actions)==1,'Reconnect must use actual TC retries, without proxy-invented callback copies')
    first=next(iter(callbacks.values()));records=journal_records(journal,first['body'])
    return dict(case=case,xid=xid,branch_id=issued,contender=contender,apply_calls=applied,effects=effects,recover_calls=recovered,
                native_RM_registrations=len([r for r in requests if r['kind']==103]),actual_TC_callbacks=len(callbacks),actual_RM_replies=sum(map(len,replies.values())),
                duplicate_deliveries=sum(r['delivery_count'] for r in callbacks.values()),frames=len(frames),journal_records=records,
                original_branch_replayed=False,end_unknown=unknown=='true',resource_fixture_only=True,SQL_Undo_verified=False)


def validate_tm(result,text,tm_rows,rm_rows,tc_log,nonce):
    """Bind direct-operation claims to a separate passthrough TM wire observer and original TC stdout bytes."""
    xid,contender=result['xid'],result['contender'];operation=result['case'].split('-')[1]
    end_lines=re.findall(r'^END mode=(commit|rollback) xid=(\S+) status=(\S+) unknown=(true|false)$',text,re.M)
    need(len(end_lines)==1 and end_lines[0][:2]==(operation,xid),'Missing unique original end diagnostic')
    unknown=end_lines[0][3]=='true';expected_end=7 if operation=='commit' else 9
    need(all(type(row.get('index')) is int and row['index']==index for index,row in enumerate(tm_rows)),'TM observer rows reordered or duplicated')
    summaries=[row for row in tm_rows if row.get('event')=='summary'];need(len(summaries)==1,'Missing unique TM observer summary')
    summary=summaries[0]
    need(summary.get('role')=='TM' and summary.get('mode')=='basic' and summary.get('succeeded') is True and summary.get('errors')==[],'TM observer did not finish passthrough')
    need(all(type(summary.get(key)) is int and summary[key]==0 for key in ['faults','released','closed']),'TM observer injected a fault')
    need(not any(row.get('event')=='error' for row in tm_rows),'TM observer recorded an error')
    tm_frames=[envelope(row) for row in tm_rows if row.get('event')=='frame'];rm_frames=[envelope(row) for row in rm_rows if row.get('event')=='frame']
    for group in [tm_rows,rm_rows]:
        ready=[row for row in group if row.get('event')=='ready'];need(len(ready)==1,'Missing clock authority record')
        clock=ready[0].get('clock',{});resolution=clock.get('resolution_seconds')
        need(clock.get('name')=='perf_counter_ns' and clock.get('monotonic') is True and clock.get('adjustable') is False and type(resolution) in [int,float] and 0<resolution<=1e-6,'Clock resolution cannot certify strict cross-process ordering')
    for group in [tm_frames,rm_frames]:
        clocks=[row.get('monotonic_ns') for row in group];need(all(type(value) is int and value>=0 for value in clocks) and clocks==sorted(clocks),'Invalid same-host observer clock sequence')
    pending={};used=set();accepted=set();requests=[];responses=[]
    expected_names={'rm-'+nonce+'-resource':xid,'rm-'+nonce+'-contender':contender}
    for row in tm_frames:
        need(row.get('role')=='TM' and row['action']=='forward' and row['delivery_count']==1,'TM frame was misrouted or transformed')
        data=row['body'];reader=Reader(data);kind=reader.number('>H') if data else None;key=(row['connection'],row['request_id'])
        if kind is None:continue
        if row['direction']=='c2s' and row['request_type']==0:
            need(kind in [101,1,7,9,15] and key not in used,'Unexpected/reused TM request ID');used.add(key)
            value=dict(kind=kind,key=key,frame=row)
            if kind==101:
                need([reader.string() for _ in range(4)]==['1.1.0','phase0-auth','default_tx_group','phase0-local-only'],'TM native role/identity differs')
            else:
                need(row['connection'] in accepted,'TM business preceded accepted registration')
                if kind==1:
                    timeout=reader.number('>i');name=reader.string();need(timeout==60000 and name in expected_names,'Unexpected Begin name/timeout');value.update(name=name,xid=expected_names[name])
                else:
                    current=reader.string();need(current in [xid,contender] and reader.string() is None,'TM changed original XID/optional extraData');value['xid']=current
            reader.done();pending[key]=value;requests.append(value)
        elif row['direction']=='s2c' and row['request_type']==1:
            need(key in pending,'TM response lacks one original request');original=pending.pop(key);need(kind==original['kind']+1,'TM response family differs')
            value=dict(kind=kind,original=original,frame=row)
            if kind==102:
                need(reader.number('>B')==1,'TM registration not accepted');reader.string();accepted.add(row['connection'])
            else:
                result_code=reader.number('>B');need(result_code in [0,1],'Invalid TM result code')
                message=reader.string() if result_code==0 else None;exception_code=reader.number('>B')
                need(exception_code in range(25),'Invalid transaction exception code')
                late_original=unknown and original['kind']==expected_end and original.get('xid')==xid
                need((result_code==1 and exception_code==0) or (result_code==0 and late_original),'A failed TM call other than the original unknown end cannot claim success')
                value.update(result_code=result_code,exception_code=exception_code,message=message)
                if kind==2:
                    need(reader.string()==original['xid'] and reader.string() is None,'TC Begin did not issue the original XID/extraData')
                else:
                    value['status']=reader.number('>B');need(value['status'] in range(21),'Invalid raw global status')
            reader.done();responses.append(value)
        else:raise ValueError('Unexpected TM callback/envelope')
    begins=[r for r in requests if r['kind']==1]
    need([r['name'] for r in begins]==list(expected_names),'Missing/duplicate original two Begin calls')
    need(len([r for r in responses if r['kind']==2])==2,'Missing actual TC-issued Begin identities')
    ends=[r for r in requests if r['kind'] in [7,9]]
    need([(r['kind'],r['xid']) for r in ends]==[(expected_end,xid),(9,contender)],'Original global end was replayed/reversed or contender cleanup changed')
    final_end=[r for r in responses if r['original'] is ends[0]]
    if not unknown:
        need(len(final_end)==1 and final_end[0]['result_code']==1 and end_lines[0][2]==str(final_end[0]['status']),'Claimed end status differs from actual matching TC response')
    else:
        need(end_lines[0][2]=='unknown','Unknown end cannot claim a global status')
    need(all(value is ends[0] and unknown for value in pending.values()),'Unanswered TM call other than the original unknown end')
    contender_end=[row for row in responses if row['original'] is ends[1]]
    need(len(contender_end)==1 and contender_end[0]['result_code']==1 and contender_end[0]['status']==11,'Fresh contender did not explicitly return Rollbacked')
    observations=[(r['original']['xid'],r['status']) for r in responses if r['kind']==16]
    claimed=[(match[0],int(match[1])) for match in re.findall(r'^OBSERVE xid=(\S+) status=(\d+)$',text,re.M)]
    need(observations==claimed and observations and observations[-1]==(contender,15),'Client status observations differ from actual paired TC responses')
    target_finished=[r for r in responses if r['kind']==16 and r['original']['xid']==xid and r['status']==15]
    need(len(target_finished)==1,'Original target cleanup not explicitly observed once')
    callbacks=[r for r in rm_frames if r['direction']=='s2c' and r['request_type']==0 and r['kind'] in [3,5]]
    locks=[r for r in rm_frames if r['direction']=='s2c' and r['kind']==22]
    delivered=[r for r in rm_frames if r['direction']=='c2s' and r['kind'] in [4,6] and r['delivery_count']==1]
    need(locks[1]['monotonic_ns']<ends[0]['frame']['monotonic_ns']<min(r['monotonic_ns'] for r in callbacks),'Global decision did not follow original lock conflict and precede TC callback')
    need(max(r['monotonic_ns'] for r in delivered)<target_finished[0]['frame']['monotonic_ns'],'Claimed Finished preceded final delivered branch completion')
    need(type(tc_log) is bytes,'TC stdout must remain raw bytes')
    received=re.findall(rb"receive msg\[single\]: GlobalBeginRequest\{transactionName='([^']+)', timeout=(\d+)\}",tc_log)
    names=[('rm-'+nonce+'-resource').encode(),('rm-'+nonce+'-contender').encode()]
    need(received==[(name,b'60000') for name in names] and len(re.findall(rb'receive msg\[single\]: GlobalBeginRequest',tc_log))==2,'TC did not independently receive the exact two named Begins')
    actual=re.findall(rb'Begin new global transaction applicationId: ([^,]+),transactionServiceGroup: ([^,]+), transactionName: ([^,]+),timeout:(\d+),xid:([^\r\n\s]+)',tc_log)
    need(actual==[(b'phase0-auth',b'default_tx_group',names[0],b'60000',xid.encode()),(b'phase0-auth',b'default_tx_group',names[1],b'60000',contender.encode())],'TC actual issued XID/name/application identity differs')
    received_ends=re.findall(rb"receive msg\[single\]: Global(Commit|Rollback)Request\{xid='([^']+)', extraData='null'\}",tc_log)
    need(received_ends==[(b'Commit' if operation=='commit' else b'Rollback',xid.encode()),(b'Rollback',contender.encode())] and len(re.findall(rb'receive msg\[single\]: Global(?:Commit|Rollback)Request',tc_log))==2,'TC independently saw extra/missing/reversed global end calls')
    return dict(global_begins=2,global_end_requests=2,target_end_requests=1,contender_rollback_requests=1,status_observations=len(observations),
                target_end_response_status=final_end[0]['status'] if final_end else None,target_end_result_code=final_end[0]['result_code'] if final_end else None,
                target_end_exception_code=final_end[0]['exception_code'] if final_end else None,target_end_message=final_end[0]['message'] if final_end else None,
                contender_rollback_observed_status=11,client_reported_end_unknown=unknown,unknown_timing_not_independently_claimed=True,
                observed_end_status_bound_to_wire=not unknown,TC_original_request_logs_bound=True,TM_wire_frames=len(tm_frames),Finished_only_cleanup_observation=True)
