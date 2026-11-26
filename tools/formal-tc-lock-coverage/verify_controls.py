"""Targeted negative controls on copies of the actual TC wire/DB/native evidence; never edit sealed observations."""
import copy
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys
import uuid

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
spec=importlib.util.spec_from_file_location('tc_lock_controls_validator',HERE/'validate.py');validator=importlib.util.module_from_spec(spec);spec.loader.exec_module(validator)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_bytes())
def validate(data):return validator.validate(**data)
def run(packet):
    for row in read(packet/'catalog.json')['members']:validator.need(sha(packet/row['path'])==row['sha256'],'Actual observation archive drift')
    data=dict(cases=read(packet/'client-artifacts/native-cases.json'),checkpoints=read(packet/'checkpoints.json'),
        rm_rows=[json.loads(r) for r in (packet/'events.jsonl').read_text('utf-8').splitlines()],
        tm_rows=[json.loads(r) for r in (packet/'tm-events.jsonl').read_text('utf-8').splitlines()],
        nonce=read(packet/'owned-runtime.json')['label_owner'],client_result=read(packet/'client-result.json'),
        native_logs={p.name:read(p) for p in (packet/'client-artifacts').glob('native-*.json') if p.name!='native-cases.json'})
    baseline=validate(data)
    def native(d,index=0,role='owner'):
        return d['native_logs'][f'native-{index}-{role}.json']['sqlUndoLogs'][1][0]['afterImage']['rows'][1]
    def frame(d,role,kind,direction):return next(r for r in d[role+'_rows'] if r.get('kind')==kind and r.get('direction')==direction)
    def replace_byte(row,offset,value):
        raw=bytearray.fromhex(row['frame_hex']);raw[offset]=value;row['frame_hex']=raw.hex()
    def change_credential(d):
        row=frame(d,'rm',103,'c2s');raw=bytes.fromhex(row['frame_hex']);validator.need(raw.count(b'phase0-local-only')==1,'Credential mutation anchor differs');row['frame_hex']=raw.replace(b'phase0-local-only',b'phase1-local-only').hex()
    def reorder_query(d):
        rows=d['rm_rows'];query=next(r for r in rows if r.get('kind')==21);reply=next(r for r in rows if r.get('kind')==22 and r['request_id']==query['request_id'] and r['connection']==query['connection'])
        rows.remove(query);rows.remove(reply);at=next(i for i,r in enumerate(rows) if r.get('kind')==11)
        query['monotonic_ns']=rows[at]['monotonic_ns']-2;reply['monotonic_ns']=rows[at]['monotonic_ns']-1;rows[at:at]=[query,reply]
        for index,row in enumerate(rows):row['index']=index
    controls=[
        ('native-pk-value',lambda d:native(d)[0]['fields'][1][0].__setitem__('value','different'),'Native actual primary values/order differ'),
        ('native-pk-jdbc-type',lambda d:native(d)[0]['fields'][1][0].__setitem__('type',4),'Native text JDBC value differs'),
        ('native-multirow-order',lambda d:native(d,4).reverse(),'Native actual primary values/order differ'),
        ('missing-owned-lock-row',lambda d:d['checkpoints'][1]['locks'].clear(),'Actual DB lock rows differ'),
        ('forged-branch-resource',lambda d:d['checkpoints'][1]['branches'][0].__setitem__('resource','different'),'Original DB global status/branch resource differs'),
        ('forged-global-state',lambda d:d['checkpoints'][1]['globals'][0].__setitem__('status',4),'Original DB global status/branch resource differs'),
        ('forged-physical-row-key',lambda d:d['checkpoints'][1]['locks'][0].__setitem__('row_key','different'),'DB physical lock identity differs'),
        ('registration-credential',change_credential,'Native registration role/application identity differs'),
        ('registration-not-identified',lambda d:replace_byte(frame(d,'rm',104,'s2c'),18,0),'Native registration not identified'),
        ('rollback-returned-committed',lambda d:replace_byte(frame(d,'tm',10,'s2c'),-1,9),'Original rollback did not explicitly return Rollbacked'),
        ('callback-returned-timeout',lambda d:replace_byte(frame(d,'rm',6,'c2s'),-1,4),'Callback identity/status differs'),
        ('finished-query-status',lambda d:replace_byte(next(r for r in d['tm_rows'] if r.get('kind')==16 and bytes.fromhex(r['frame_hex'])[-1]==15),-1,11),'Final cleanup query missing or precedes original end'),
        ('query-before-owner-lock',reorder_query,'Owner observation/contender conflict-query/registration ordering differs'),
        ('owner-snapshot-after-query',lambda d:d['checkpoints'][1].__setitem__('observer_completed_ns',frame(d,'rm',21,'c2s')['monotonic_ns']+1),'Owner observation/contender conflict-query/registration ordering differs'),
        ('client-end-status-forged',lambda d:d['checkpoints'][3]['data']['owner_end'].__setitem__('status',9),'Client original end differs from actual response'),
        ('cleanup-lock-row-remains',lambda d:d['checkpoints'][3]['locks'].append(copy.deepcopy(d['checkpoints'][1]['locks'][0])),'Original TC storage was not released'),
    ]
    results=[]
    for name,mutate,expected in controls:
        changed=copy.deepcopy(data);mutate(changed)
        try:validate(changed)
        except ValueError as error:
            validator.need(str(error)==expected,name+' hit unrelated rejection: '+str(error));results.append(dict(control=name,actual_rejection=str(error),input_copy_only=True))
        else:raise ValueError(name+' incorrectly accepted altered actual evidence')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/tc-lock-controls'/('run_'+stamp);out.mkdir(parents=True)
    def save(name,value):(out/name).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    frozen=[]
    for path in [Path(__file__),HERE/'validate.py',HERE.parent/'formal-rm-java/validate.py']:
        leaf=hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';(out/leaf).write_bytes(path.read_bytes());frozen.append(dict(path=str(path),snapshot=leaf,sha256=sha(path)))
    save('source-inputs.json',frozen);save('result.json',dict(status='ACTUAL_EVIDENCE_NEGATIVE_CONTROLS_VERIFIED',observation_packet=str(packet),observation_catalog_sha256=sha(packet/'catalog.json'),baseline=baseline,negative_controls=results,controls=len(results),TC_safety_gate_passed=False))
    for row in frozen:validator.need(sha(Path(row['path']))==row['sha256'],'Control source drift')
    save('catalog.json',dict(members=[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]));print(json.dumps(dict(run=str(out),controls=len(results),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':run(Path(sys.argv[1]))
