from copy import deepcopy
from pathlib import Path
import importlib.util
import json
import re
import struct
import sys

def load(name, path):
    spec=importlib.util.spec_from_file_location(name, path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module

root=Path.cwd()
wire=load('owned_raw_java_runtime', root/'tools/formal-transport-loss/live.py')
journal=load('owned_file_journal', root/'tools/formal-rm-java/validate.py')
folder=Path(sys.argv[1])
out=folder/'java-tc'
observed=json.loads((folder/'observations/java-rollback-acquisition.json').read_bytes())
def require(test, reason):
    if not test:
        raise ValueError(reason)

require((folder/'observations/normal-before-fault.json').read_bytes()==
        (folder/'observations/normal-after-fault.json').read_bytes(),'independent Normal Undo snapshots differ')

def read(role):
    return [json.loads(line) for line in (out/f'{role}-events.jsonl').read_text('utf-8').splitlines()]

def validate(events, witness, log):
    frames={}
    for role,rows in events.items():
        require(all(row['index']==i for i,row in enumerate(rows)), 'raw observer sequence')
        require(rows[-1]['event']=='summary' and rows[-1]['succeeded'] is True and not rows[-1]['errors'] and rows[-1]['faults']==0,'raw observer summary')
        frames[role]=[r for r in rows if r['event']=='frame']
        for row in frames[role]:
            raw=bytes.fromhex(row['frame_hex'])
            require(raw[:3]==b'\xda\xda\x01' and struct.unpack('>I',raw[3:7])[0]==len(raw)
                    and raw[7:9]==b'\x00\x10' and raw[10:12]==b'\x01\x00'
                    and (raw[9],struct.unpack('>i',raw[12:16])[0])==(row['request_type'],row['request_id'])
                    and (struct.unpack('>H',raw[16:18])[0] if len(raw)>16 else None)==row['kind']
                    and row['action']=='forward' and row['delivery_count']==1,'unaltered Seata envelope')
    def select(role,kind,direction):
        return [r for r in frames[role] if r['kind']==kind and r['direction']==direction]
    request,=select('RM',11,'c2s')
    allocated,=select('RM',12,'s2c')
    original=wire.branch_identity(request,False)
    require(original['xid']==witness['xid'] and original['resource']==witness['resource_id'] and
            original['branch_type']==0 and original['data']=='' and original['lock']=='undo_work:1','registration identity')
    value=wire.payload(allocated)
    require(len(value)==12 and value[:4]==b'\x00\x0c\x01\x00' and struct.unpack('>q',value[4:])[0]==witness['branch_id'] and
            (allocated['connection'],allocated['request_id'])==(request['connection'],request['request_id']),'original allocation')

    callbacks=select('RM',5,'s2c')
    replies=select('RM',6,'c2s')
    require(len(callbacks)==len(replies)==2, 'one original rollback callback and one TC retry required')
    require(callbacks[0]['request_id']!=callbacks[1]['request_id'],'different original callback correlation ids')
    require(all(c['connection']==request['connection'] for c in callbacks),'same registered RM generation')
    for callback,reply,status in zip(callbacks,replies,[9,8]):
        value=wire.branch_identity(callback,True)
        require(value['xid']==witness['xid'] and value['branch_id']==witness['branch_id'] and
                value['resource']==witness['resource_id'] and value['data']=='' and value['branch_type']==0,'original callback identity')
        require((reply['connection'],reply['request_id'])==(callback['connection'],callback['request_id']),'original callback reply correlation')
        payload=wire.payload(reply)
        xid,at=wire.string(payload,4)
        require(payload[:4]==b'\x00\x06\x01\x00' and len(payload)==at+9 and
                (xid,struct.unpack('>q',payload[at:at+8])[0],payload[-1])==(witness['xid'],witness['branch_id'],status),'retryable then completed callback status')
    records=journal.journal_records(log,wire.payload(callbacks[0]))
    end,=select('TM',9,'c2s')
    response,=select('TM',10,'s2c')
    require(wire.string(wire.payload(end),2)[0]==witness['xid'] and (end['connection'],end['request_id'])==
            (response['connection'],response['request_id']) and wire.payload(response)[2:]==b'\x01\x00\x05','one TM rollback and Retrying response')
    require(end['monotonic_ns']<callbacks[0]['monotonic_ns']<replies[0]['monotonic_ns']<callbacks[1]['monotonic_ns']<replies[1]['monotonic_ns'],'actual callback ordering')
    status_requests=select('TM',15,'c2s')
    statuses=select('TM',16,'s2c')
    requests_by_id={(row['connection'],row['request_id']):row for row in status_requests}
    require(len(requests_by_id)==len(status_requests)==len(statuses)>0,'original TC status request count')
    for status in statuses:
        query=requests_by_id.pop((status['connection'],status['request_id']))
        require(wire.string(wire.payload(query),2)[0]==witness['xid'] and
                query['monotonic_ns']<status['monotonic_ns'],'TC status request keeps original XID and correlation')
    require(not requests_by_id and wire.payload(statuses[-1])[2:]==b'\x01\x00\x0f' and
            replies[1]['monotonic_ns']<statuses[-1]['monotonic_ns'],'finished original TC status after rollback reply')
    require(not select('TM',7,'c2s') and not select('RM',13,'c2s'),'no client commit or synthetic report')
    require(witness['fault_connection_after_arm']==3 and witness['physical_acquisitions_after_arm']==6 and
            witness['first']==dict(origin=1,phase=2,terminal=0,undo_preserved=True,business_rows=1) and
            witness['final']==dict(origin=1,phase=2,terminal=1,file='Completed',undo_rows=0,business_rows=0),'physical database observation')
    return dict(original_callbacks=2,original_branch=witness['branch_id'],original_tm_rollbacks=1,
                callback_statuses=[9,8],journal_states=[r['state'] for r in records])

events={role:read(role) for role in ('RM','TM')}
raw_journal=(folder/'observations/java-branches.log').read_bytes()
result=validate(events,observed,raw_journal)
controls=[]
def negative(name,variant=events,witness=observed,log=raw_journal):
    try:
        validate(variant,witness,log)
    except (ValueError,IndexError,KeyError,struct.error):
        controls.append(name)
        return
    raise SystemExit('weakened case accepted: '+name)

variant=deepcopy(events)
first=next(row for row in variant['RM'] if row.get('kind')==6)
payload=bytearray.fromhex(first['frame_hex']);payload[-1]=8
first['frame_hex']=payload.hex()
negative('first-reply-changed-to-terminal',variant)
variant=deepcopy(events)
def without_retry(rows):
    count={5:0,6:0}
    result=[]
    for row in rows:
        kind=row.get('kind')
        if kind in count:
            count[kind]+=1
            if count[kind]==2:
                continue
        result.append(dict(row,index=len(result)))
    return result
variant['RM']=without_retry(variant['RM'])
negative('tc-retry-and-reply-removed',variant)
negative('changed-original-branch',witness=observed|{'branch_id':observed['branch_id']^1})
variant=deepcopy(events)
query=next(row for row in reversed(variant['TM']) if row.get('kind')==15)
payload=bytearray.fromhex(query['frame_hex']);payload[20]=ord('X')
query['frame_hex']=payload.hex()
negative('final-status-queried-for-another-xid',variant)
negative('changed-journal-crc',log=raw_journal[:-1]+bytes([raw_journal[-1]^1]))
binding=json.loads((out/'owned-processes.json').read_bytes())
require(binding['jar_sha256']==wire.reg.JAR_SHA and binding['java_pin']==wire.reg.PIN,'pinned runtime identity')
log=(out/'class-load.log').read_text('utf-8')
origin='org.apache.seata.server.coordinator.DefaultCoordinator'
require(any(origin+' source:' in line and 'seata-server.jar' in line for line in log.splitlines()),'loaded original coordinator')
result['java_pin']=binding['java_pin']
result['negative_controls']=controls
(out/'rollback-retry-validation.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
print(json.dumps(result))
