"""独立原始V1字节验收；客户端声明、proxy元数据与TC真正身份/状态必须分别吻合。"""
import importlib.util
import json
from pathlib import Path
import re
import struct

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('tm_loss_wire_support',ROOT/'tools/formal-transport-loss/live.py')
support=importlib.util.module_from_spec(spec);spec.loader.exec_module(support)


def require(value,reason):
    if not value:raise ValueError(reason)


def registration(rows):
    identities={}
    for kind,role in [(101,'TM'),(103,'RM')]:
        selected=[(i,row) for i,row in enumerate(rows) if row.get('event')=='frame' and row['direction']=='c2s' and row['kind']==kind]
        require(len(selected)==1,'Need exactly one native '+role+' registration')
        index,row=selected[0];data=support.payload(row);at=2;values=[]
        for _ in range(4):value,at=support.string(data,at);values.append(value)
        require(values==['1.1.0','phase0-auth','default_tx_group','phase0-local-only'],'Native identity differs')
        if role=='RM':
            resource,at=support.string(data,at,True);require(resource=='mysql://auth-a','Original RM resource differs')
        require(at==len(data),'Native identity trailing bytes')
        replies=[(i,r) for i,r in enumerate(rows) if r.get('matched_request_index')==index and r.get('kind')==kind+1]
        require(len(replies)==1 and not replies[0][1]['dropped'],'Missing unique native identity confirmation')
        response=support.payload(replies[0][1]);require(response[2:3]==b'\x01','Native identity rejected')
        _,end=support.string(response,3);require(end==len(response),'Invalid native identity confirmation')
        identities[role]=(row['connection'],replies[0][0])
    require(identities['TM'][0]!=identities['RM'][0],'TM and RM must use their own physical connections')
    for index,row in enumerate(rows):
        if row['event']!='frame':continue
        require(row['connection'] in [identities['TM'][0],identities['RM'][0]],'Frame migrated to an unregistered connection')
        if row['direction']=='c2s' and row['request_type']==0 and row['kind'] not in [101,103]:
            role='RM' if row['kind'] in [11,21] else 'TM'
            require(row['connection']==identities[role][0] and index>identities[role][1],'Business frame preceded native identity or used wrong role')
    return identities


def validate(mode,text,original,nonce):
    require(mode in ['begin','commit','rollback'],'Unknown TM loss mode')
    rows=support.validate_wire_rows(original)
    frames=[(i,row) for i,row in enumerate(rows) if row['event']=='frame']
    completed=set()
    for _,row in frames:
        if row['request_type']==1:
            key=(row['direction'],row['connection'],row['request_id'],row['matched_request_index'])
            require(key not in completed,'An original request has more than one actual response')
            completed.add(key)
    summaries=[row for row in rows if row['event']=='summary']
    require(len(summaries)==1 and summaries[0]['succeeded'] and summaries[0]['drop_count']==1 and not summaries[0].get('errors') and sorted(summaries[0]['roles'])==['RM','TM'],'Proxy did not complete one clean loss scenario')
    require(not any(row['event']=='error' for row in rows),'Proxy error event present')
    roles=registration(rows)
    dropped=[(i,row) for i,row in frames if row['dropped']]
    require(len(dropped)==1 and dropped[0][1]['direction']=='s2c' and dropped[0][1]['request_type']==1 and dropped[0][1]['kind']=={'begin':2,'commit':8,'rollback':10}[mode],'Wrong original response loss')
    drop_index,drop=dropped[0];target=drop['xid'];require(bool(target),'Lost response must identify original transaction independently')
    requests=[(i,row) for i,row in frames if row['direction']=='c2s' and row['request_type']==0]
    begins=[(i,row) for i,row in requests if row['kind']==1]
    begin_replies=[row for _,row in frames if row['kind']==2 and row['direction']=='s2c']
    require(len(begins)==len(begin_replies),'A Begin has no actual successful response')
    for _,row in begins:
        data=support.payload(row);require(struct.unpack('>i',data[2:6])[0]==60000,'TC timeout replaced by transport budget')
        name,end=support.string(data,6);require(end==len(data) and name.startswith('tm-loss-'+nonce+'-'),'Original transaction name/nonce differs')
    if mode=='begin':
        require(len(begins)==1 and all(row['kind'] in [101,103,1] for _,row in requests),'Lost Begin was blindly repeated or observer identity used in recovery')
        match=re.fullmatch(r'PASS TM_LOSS_BEGIN operation=1 request_id=(-?\d+) xid_known=false begin_attempts=1 business_started=false unknown=true\s*',text)
        require(match is not None and int(match[1])==drop['request_id'] and target not in text,'Begin leaked observer XID, started business or lost original error')
        return dict(mode=mode,drop_count=1,observer_only_xid=target,client_xid_known=False,business_started=False,begin_requests=1)
    commit=mode=='commit';operation=7 if commit else 9;terminal=9 if commit else 11
    require(drop['global_status']==terminal,'Withheld original end response was not actual success')
    require(len(begins)==2 and len(set(row['xid'] for row in begin_replies))==2,'Expected distinct contender and target Begin only')
    by_xid={row['xid']:row for row in begin_replies};require(target in by_xid,'Target was not created by the formal TM')
    contender=next(xid for xid in by_xid if xid!=target)
    require(all(row['kind'] in [101,103,1,7,9,11,15,21] for _,row in requests),'Unexpected mutating operation in TM fault driver')
    original_ends=[(i,row) for i,row in requests if row['kind'] in [7,9] and row['xid']==target]
    require(len(original_ends) in [1,2] and all(row['kind']==operation for _,row in original_ends),'Recovery switched operation or replayed more than one explicit duplicate')
    first_end=original_ends[0][0];require(drop['matched_request_index']==first_end,'Drop not correlated to first original end request')
    regs=[(i,row) for i,row in requests if row['kind']==11]
    require(len(regs)==1 and regs[0][1]['xid']==target,'Expected one original branch registration')
    reg_index,reg=regs[0];identity=support.branch_identity(reg,False)
    require(identity['branch_type']==0 and identity['resource']=='mysql://auth-a' and identity['lock']=='orders:1' and json.loads(identity['data'])=={'formalOperation':nonce},'Branch identity/resource/lock/operation nonce differs')
    registered=[(i,row) for i,row in frames if row['kind']==12 and row['matched_request_index']==reg_index]
    require(len(registered)==1 and registered[0][0]<first_end and not registered[0][1]['dropped'],'Original branch was not successfully registered before end')
    branch_id=registered[0][1]['branch_id']
    callbacks=[(i,row) for i,row in frames if row['direction']=='s2c' and row['request_type']==0]
    replies=[(i,row) for i,row in frames if row['direction']=='c2s' and row['request_type']==1]
    require(len(callbacks)==len(replies)==1,'Expected one actual branch callback and reply')
    callback_index,callback=callbacks[0];reply_index,reply=replies[0]
    cb=support.branch_identity(callback,True)
    require(callback['kind']==(3 if commit else 5) and callback['connection']==roles['RM'][0] and cb['xid']==target and cb['branch_id']==branch_id and cb['resource']==identity['resource'] and cb['branch_type']==0 and cb['data']==identity['data'],'Callback changed original identity, intent or RM connection')
    require(reply['matched_request_index']==callback_index and first_end<callback_index<reply_index,'Callback reply did not retain originating wire/connection order')
    if not commit:require(reply_index<dropped[0][0],'Successful Rollback preceded its actual branch reply')
    before=[row for i,row in frames if row['kind']==16 and row['xid']==target and i<first_end]
    require(before and before[-1]['global_status']==1,'Original target not observed in Begin before explicit scoped end')
    end_limit=original_ends[1][0] if len(original_ends)==2 else len(rows)
    after=[(i,row) for i,row in frames if row['kind']==16 and row['xid']==target and reply_index<i<end_limit]
    require(after,'Same-operation recovery has no original-XID status observation')
    for index,row in after:
        query_index=row['matched_request_index'];query=rows[query_index]
        require(drop_index<query_index<index<end_limit and query['kind']==15 and query['xid']==target,'Recovery status request was already in flight before the original response loss')
    allowed=[2,3,8,9,15] if commit else [4,5,6,7,11,13,15]
    require(all(row['global_status'] in allowed and not row['dropped'] for _,row in after),'Recovery observed opposite decision or operator state')
    retry=[]
    if len(original_ends)==2:
        require(all(row['global_status'] not in ([9] if commit else [11,13]) for _,row in after),'Duplicate sent despite an already decisive matching status')
        retry=[row for _,row in frames if row['kind']==operation+1 and row['matched_request_index']==original_ends[1][0]]
        require(len(retry)==1 and not retry[0]['dropped'] and retry[0]['global_status'] in ([9,15] if commit else [11,13,15]),'Explicit same-operation duplicate did not return a valid TC result')
    final_status=after[-1][1]['global_status'];retry_status=retry[0]['global_status'] if retry else -1
    known=any(row['global_status'] in ([9] if commit else [11,13]) for _,row in after) or retry_status in ([9] if commit else [11,13])
    require(final_status in ([9,15] if commit else [11,13,15]),'Final query did not reach a bounded fixture end observation')
    require(len(original_ends)==2 or known,'Unresolved end was neither reconciled nor explicitly retried')
    queries=[(i,row) for i,row in requests if row['kind']==21];locks=[(i,row) for i,row in frames if row['kind']==22]
    require(len(queries)==len(locks)==2 and support.lock_results([row for _,row in locks])==[0,1],'Branch lock was not occupied then released')
    for (index,query),(response_index,response) in zip(queries,locks):
        lock_identity=support.branch_identity(query,False)
        require(query['xid']==contender and (lock_identity['resource'],lock_identity['lock'],lock_identity['branch_type'])==(identity['resource'],identity['lock'],0) and response['matched_request_index']==index and index<response_index,'Lock observation changed contender/resource/key/wire correlation')
    require(registered[0][0]<queries[0][0]<locks[0][0]<first_end and reply_index<queries[1][0],'Lock/end/callback sequence differs')
    cleanup=[(i,row) for i,row in requests if row['kind'] in [7,9] and row['xid']!=target]
    require(len(cleanup)==1 and cleanup[0][1]['kind']==9 and cleanup[0][1]['xid']==contender and cleanup[0][0]>locks[1][0],'Unexpected mutation or missing explicit contender cleanup')
    cleaned=[row for _,row in frames if row['kind']==10 and row['matched_request_index']==cleanup[0][0]]
    require(len(cleaned)==1 and cleaned[0]['global_status']==11 and not cleaned[0]['dropped'],'Contender rollback not explicitly acknowledged')
    claim=re.search(r'PASS TM_LOSS_(COMMIT|ROLLBACK) operation=(7|9) request_id=(-?\d+) xid=(\S+) branch_id=(\d+) callback_id=(-?\d+) attempts=(1|2) explicit_retry=(true|false) observed=(\d+) retry_status=(-?\d+) unknown_retained=(true|false) actions=1 business_error_preserved=(true|false)',text)
    require(claim is not None,'Missing exact original formal TM uncertainty claim')
    expected=(mode.upper(),operation,drop['request_id'],target,branch_id,callback['request_id'],len(original_ends),len(original_ends)==2,final_status,retry_status,not known,not commit)
    actual=(claim[1],int(claim[2]),int(claim[3]),claim[4],int(claim[5]),int(claim[6]),int(claim[7]),claim[8]=='true',int(claim[9]),int(claim[10]),claim[11]=='true',claim[12]=='true')
    require(actual==expected,'Client uncertainty/identity/status/business claim differs from actual raw responses')
    return dict(mode=mode,drop_count=1,xid=target,branch_id=branch_id,end_attempts=len(original_ends),original_operation=operation,observed=final_status,retry_status=retry_status,unknown_retained=not known,
        callback_id=callback['request_id'],phase_two_actions=1,lock_released=True,business_error_preserved=not commit)
