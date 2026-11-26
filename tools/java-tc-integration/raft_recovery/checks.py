"""Strict evidence gates for original identities, real elections and asynchronous reload exceptions."""
import datetime
import time

def verify_records(state,status,host,after_election=False):
    records=state.get('records',[])
    if len(records)!=2:raise RuntimeError('Incomplete original record inventory')
    for index,record in enumerate(records):
        expected=dict(xid=f'{host}:28091:{940000+index}',resource=f'mysql://raft-retry-{index}',lockable=index!=0)
        if any(record.get(key)!=value for key,value in expected.items()):raise RuntimeError('Original identity/resource/lock differs')
        if status==1 and after_election and index==1:
            if record.get('exists') is not False:raise RuntimeError('Empty Begin control did not complete after leader reload')
            continue
        expected.update(exists=True,status=5 if status==1 and after_election else status,
                        branches=1 if index==0 else 0,branch_id=950000 if index==0 else 0,branch_type=0 if index==0 else -1,
                        branch_resource=f'mysql://raft-retry-{index}' if index==0 else None,
                        branch_lock_key='orders:1' if index==0 else None,branch_xid=f'{host}:28091:{940000+index}' if index==0 else None)
        if any(record.get(key)!=value for key,value in expected.items()):raise RuntimeError('Original persisted state/branch identity differs')
    if state.get('all_sessions')!=(1 if after_election and status==1 else 2):raise RuntimeError('Unexpected extra or missing sessions')
    return True

def verify_transition(before,after,stopped_node):
    if (after.get('leader') is not True or after.get('node_id')==stopped_node
            or after.get('term',0)<=before['term']
            or after.get('committed',-1)<before['committed'] or after.get('applied',-1)<before['committed']):
        raise RuntimeError('No proven real leader transition with replicated state')
    return True

def reload_exceptions(events,status,after=None):
    name={16:'CommitRetryTimeout',17:'RollbackRetryTimeout'}.get(status)
    if status not in [1,16,17]:return []
    found=[]
    for event in events:
        values=event.get('values',{})
        thrown=values.get('thrownClass') or {}
        frames=(values.get('stackTrace') or {}).get('frames') or []
        methods=[(frame.get('method',{}).get('type',{}).get('name','').replace('/','.'),frame.get('method',{}).get('name','')) for frame in frames]
        if after is not None:
            try:
                if datetime.datetime.fromisoformat(values.get('startTime','').replace('Z','+00:00'))<=datetime.datetime.fromisoformat(after.replace('Z','+00:00')):continue
            except (ValueError,TypeError):continue
        if (event.get('type')=='jdk.JavaExceptionThrow'
                and thrown.get('name','').replace('/','.')=='org.apache.seata.common.exception.ShouldNeverHappenException'
                and (status==1 or values.get('message')=='NOT properly handled '+name)
                and ('org.apache.seata.server.session.SessionHolder','reload') in methods
                and any(owner=='org.apache.seata.server.cluster.raft.RaftStateMachine' and method.startswith('lambda$onLeaderStart$') for owner,method in methods)):
            found.append(event)
    return found

def wait_for_reload(read_events,status,after,max_attempts=3,delay=0):
    deadline=time.monotonic()+20
    for attempt in range(max_attempts):
        found=reload_exceptions(read_events(attempt),status,after)
        if found:return found,attempt
        if time.monotonic()>=deadline:break
        if delay:time.sleep(delay)
    raise RuntimeError('No exact async reload event in this transition window before the bounded deadline')
