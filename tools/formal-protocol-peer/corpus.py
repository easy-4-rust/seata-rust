"""从实际Java语义/独立侧车构造公共字段树；不调用Rust/Go编解码器生成预期。"""
import json
from pathlib import Path
import re
import struct

ROOT=Path(__file__).resolve().parents[2]
ORACLE=Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
ERRORS='Unknown BeginFailed LockKeyConflict IO BranchRollbackFailed_Retriable BranchRollbackFailed_Unretriable BranchRegisterFailed BranchReportFailed LockableCheckFailed BranchTransactionNotExist GlobalTransactionNotExist GlobalTransactionNotActive GlobalTransactionStatusInvalid FailedToSendBranchCommitRequest FailedToSendBranchRollbackRequest FailedToAddBranch FailedLockGlobalTransaction FailedWriteSession FailedStore NotRaftLeader LockKeyConflictFailFast TransactionTimeout CommitHeuristic Broken BranchPrepareFailed'.split()
RESULT_TYPES={2,4,6,8,10,12,14,16,18,22,102,104}


def text(value):
    if value is None:return None
    if not isinstance(value,str):raise ValueError('Expected Java string/null')
    return value.encode('utf-8').hex()


def model(kind,*fields):return {'type':kind,'fields':list(fields)}
def batch(children,ids=()):return model(59,list(children),list(ids))
def results(children):return model(60,list(children))
def rpc(kind,id,body,headers=()):return {'id':id,'kind':kind,'codec':1,'compressor':0,'headers':list(headers),'body':body}


def exception_model(kind,code):
    prefix=[0,text('wire:'+ERRORS[code]),code]
    if kind==2:return model(kind,*prefix,text('127.0.0.1:8091:33'),text('meta'))
    if kind in [4,6]:return model(kind,*prefix,text('127.0.0.1:8091:33'),0x0102030405060708,6 if kind==4 else 9)
    if kind in [8,10,16,18]:return model(kind,*prefix,5)
    if kind==12:return model(kind,*prefix,0x0102030405060708)
    if kind==14:return model(kind,*prefix)
    if kind==22:return model(kind,*prefix,False)
    raise ValueError('Unknown exception response')


def build():
    inputs={}
    def read(path):
        path=Path(path);data=path.read_bytes()
        if path in inputs and inputs[path]!=data:raise ValueError('Input changed during read')
        inputs[path]=data
        return data
    def load(path):return json.loads(read(path))
    fixture=ROOT/'tests/fixtures/protocol/v1'
    authority=load(fixture/'formal_frames/certification.json')
    java=load(ROOT/authority['evidence']/'java.json')['frames']
    def enum(path,count):
        values={name:int(code) for name,code in re.findall(r'^\s+(\w+)\((\d+)[,)]',read(path).decode(),re.M)}
        if len(values)!=count:raise ValueError('Java enum source identity differs')
        return values
    globals_=enum(ORACLE/'common/src/main/java/org/apache/seata/core/model/GlobalStatus.java',21)
    branches=enum(ORACLE/'core/src/main/java/org/apache/seata/core/model/BranchStatus.java',16)
    modes={'AT':0,'TCC':1,'SAGA':2,'XA':3,'SAGA_ANNOTATION':4}
    def java_model(v):
        kind=v['typeCode'];prefix=[]
        if kind in RESULT_TYPES-{102,104}:
            prefix=[{'Failed':0,'Success':1}[v['resultCode']],text(v['msg']),ERRORS.index(v['transactionExceptionCode'])]
        if kind==1:fields=[v['timeout'],text(v['transactionName'])]
        elif kind==2:fields=prefix+[text(v['xid']),text(v['extraData'])]
        elif kind in [3,5]:fields=[text(v['xid']),v['branchId'],modes[v['branchType']],text(v['resourceId']),text(v['applicationData'])]
        elif kind in [4,6]:fields=prefix+[text(v['xid']),v['branchId'],branches[v['branchStatus']]]
        elif kind in [7,9,15]:fields=[text(v['xid']),text(v['extraData'])]
        elif kind==17:fields=[text(v['xid']),text(v['extraData']),None if v['globalStatus'] is None else globals_[v['globalStatus']]]
        elif kind in [8,10,16,18]:fields=prefix+[globals_[v['globalStatus']]]
        elif kind in [11,21]:fields=[text(v['xid']),modes[v['branchType']],text(v['resourceId']),text(v['lockKey']),text(v['applicationData'])]
        elif kind==12:fields=prefix+[v['branchId']]
        elif kind==13:fields=[text(v['xid']),v['branchId'],branches[v['status']],text(v['resourceId']),text(v['applicationData']),modes[v['branchType']]]
        elif kind==14:fields=prefix
        elif kind==22:fields=prefix+[v['lockable']]
        elif kind==101:fields=[text(v[key]) for key in ['version','applicationId','transactionServiceGroup','extraData']]
        elif kind==103:fields=[text(v[key]) for key in ['version','applicationId','transactionServiceGroup','extraData','resourceIds']]
        elif kind in [102,104]:fields=[v['identified'],text(v['version']),text(v['version'] or '2.8.0-SNAPSHOT')]
        elif kind==111:fields=[modes[v['branchType']],text(v['resourceId']),v['saveDays']]
        else:raise ValueError('Unmodeled Java type')
        return model(kind,*fields)
    by_name={row['name']:java_model(row['body']) for row in java[:29]}
    if len(by_name)!=29:raise ValueError('Need29 original Java bodies')
    base=[];types=set()
    for row in java[:29]:
        value=by_name[row['name']]
        if value['type'] not in types:types.add(value['type']);base.append(value)
    if len(base)!=25:raise ValueError('Need25 base types')
    cases=[]
    def add(id,kind,wire,semantic,accepted=True):
        cases.append({'id':id,'kind':kind,'hex':wire.hex(),'accepted':accepted,'semantic':semantic})
    for name,value in by_name.items():add('ordinary:'+name,'payload',read(fixture/(name+'.bin')),value)
    errors_path=ROOT/'docs/phase-0/evidence/local-go-java-exception-profile/run_20261112_184713_f94f053f/capture/vectors.json'
    errors=load(errors_path)
    for row in errors:
        if row['error_name']!=ERRORS[row['error_code']]:raise ValueError('Exception name/code differs')
        add('exception:'+row['name'],'payload',bytes.fromhex(row['serialized_hex']),exception_model(row['type_id'],row['error_code']))
    states=load(ROOT/'docs/phase-0/evidence/local-global-status-contract/run_20261113_130921_ed3008c9/java.json')['responses']
    java_factory=read(ROOT/'tools/java-global-status-contract/GlobalStatusContractProbe.java').decode()
    if '状态诊断:IO' not in java_factory:raise ValueError('Independent Java failed-state message source changed')
    for row in states:
        failed=row['result']==0
        if row['error']!=(3 if failed else 0):raise ValueError('State error prefix differs')
        expected=model(row['type'],row['result'],text('状态诊断:IO') if failed else None,3 if failed else 0,row['status'])
        add(f"state:{row['type']}:{row['status']}:{row['result']}",'payload',bytes.fromhex(row['hex']),expected)
    failure=load(fixture/'failure_variants/manifest.json')
    for row in failure['samples']:
        v=row['input'];kind=row['type_id'];fields=[0,text(v['msg']) or None,v['java_exception_code']]
        if kind==6:fields += [text(v['xid']),v['branch_id'],v['branch_status']]
        elif kind in [10,16,18]:fields += [v['global_status']]
        elif kind==12:fields += [v['branch_id']]
        elif kind==22:fields += [v['lockable']]
        elif kind!=14:raise ValueError('Unexpected failure layout')
        add('failure:'+row['name'],'payload',read(fixture/'failure_variants'/row['payloads']['java']['file']),model(kind,*fields))
    manifest=load(fixture/'handler_matrix/manifest.json')
    handler_types={'BEGIN':2,'COMMIT':8,'ROLLBACK':10,'STATUS':16,'REPORT':18,'REGISTER':12,'BRANCH_REPORT':14,'LOCK':22,'REG_TM':102,'REG_RM':104}
    for row in manifest['samples']:
        col=read(fixture/'handler_matrix'/row['expectation']).decode().rstrip('\n').split('\t')
        op=col[0];kind=handler_types[op]
        fields=[int(col[1]),col[3] or None,int(col[2])]
        if kind==2:fields += [col[5] or None,None]
        elif kind in [8,10,16,18]:fields += [int(col[4])]
        elif kind==12:fields += [int(col[6])]
        elif kind==22:fields += [bool(int(col[7]))]
        elif kind in [102,104]:fields=[bool(int(col[1])),col[8] or None,col[8] or text('2.8.0-SNAPSHOT')]
        add('handler:'+row['case_id'],'payload',read(fixture/'handler_matrix'/row['file']),model(kind,*fields))
    begin=model(1,2000,text('m-a'));status=model(15,text('a'),None)
    requests=batch([begin,status]);with_ids=batch([begin,status],[7,-9])
    control={'MergeRequestWithIds':with_ids,'MergeRequestWithoutIds':requests,'MergeRequestEmpty':batch([]),
        'MergeRequestNested':batch([requests],[42]),'MergeResult':results([model(2,1,None,0,text('a'),None),model(8,1,None,0,9)]),'MergeResultEmpty':results([])}
    merged={'Single_'+str(v['type']):batch([v],[-100-v['type']]) for v in base}
    all25=batch(base,[i+1 if i%2==0 else -i-1 for i in range(25)])
    all12=results([v for v in base if v['type'] in RESULT_TYPES])
    merged.update(All25WithIds=all25,All25WithoutIds=batch(base),All12Results=all12,
        All250ExceptionResults=results([exception_model(kind,code) for kind in [2,4,6,8,10,12,14,16,18,22] for code in range(25)]),
        NestedAll=batch([all25,all12],[77,-77]),AbsentTMVersion=batch([model(102,False,None,text('2.8.0-SNAPSHOT'))]),
        AbsentRMVersion=batch([model(104,False,None,text('2.8.0-SNAPSHOT'))]),
        NullReportStatus=batch([model(17,text('ctx'),text('meta'),None)]),UnknownReportStatus=batch([model(17,text('ctx'),text('meta'),0)]))
    for timeout in [-1,-2147483648,0,2147483647]:merged['SignedTimeout_'+str(timeout)]=batch([model(1,timeout,text('signed'))])
    for family,expected in [('merge_profile',merged),('control_envelopes',control)]:
        for name,value in expected.items():add(f'{family}:{name}','payload',read(fixture/family/(name+'.payload.bin')),value)
    original_frames=load(ROOT/'tools/go-golden-vectors/frames.json')
    for row in original_frames:
        headers=[[text('traceId'),text('trace-001')]] if row['name']=='GlobalBeginRequestWithHeadMapFrame' else []
        add('original-frame:'+row['name'],'frame',bytes.fromhex(row['frame_hex']),rpc(row['request_type'],row['request_id'],by_name[row['source_vector']],headers))
    controls=load(fixture/'control_envelopes/vectors.json')
    for row in controls:
        name=row['name'];body=control.get(name);headers=[[text('traceId'),text('hb')]] if name=='HeartbeatHeadMap' else []
        add('control-frame:'+name,'frame',bytes.fromhex(row['frame_hex']),rpc(row['kind'],row['request_id'],body,headers))
    for row in java:
        headers=[[text(key),text(value)] for key,value in row['headers'].items()]
        add('new-java-frame:'+row['name'],'frame',bytes.fromhex(row['frame_hex']),rpc(row['kind'],row['id'],java_model(row['body']),headers))
    if len(cases)!=628:raise ValueError('Need all628 positive contexts')
    # 通用线缆畸形比较；不把两种完整帧/流接口的错误种类强行说成相同。
    for row in list(cases):
        if row['id'].startswith(('ordinary:','control_envelopes:','original-frame:','control-frame:')):
            wire=bytes.fromhex(row['hex'])
            for length in range(len(wire)):add(f"bad-prefix:{row['id']}:{length}",row['kind'],wire[:length],None,False)
            add('bad-tail:'+row['id'],row['kind'],wire+b'\0',None,False)
    for kind in [0,19,20,23,105,106,120,121,65535]:add('bad-type:'+str(kind),'payload',struct.pack('>H',kind),None,False)
    for value in range(21,256):add('bad-status:'+str(value),'payload',bytes([0,16,1,0,value]),None,False)
    for value in range(25,256):add('bad-exception:'+str(value),'payload',bytes([0,14,1,value]),None,False)
    one=bytes.fromhex(original_frames[0]['frame_hex'])
    for index,value in [(0,0),(1,0),(2,0),(2,2),(9,5),(10,2),(10,255),(11,1),(11,255)]:
        bad=bytearray(one);bad[index]=value;add(f'bad-header:{index}:{value}','frame',bytes(bad),None,False)
    for length in [0,15,16,len(one)-1,len(one)+1,0xffffffff]:
        bad=bytearray(one);bad[3:7]=struct.pack('>I',length);add(f'bad-full:{length}','frame',bytes(bad),None,False)
    for length in [0,15,len(one)+1,32768,65535]:
        bad=bytearray(one);bad[7:9]=struct.pack('>H',length);add(f'bad-head:{length}','frame',bytes(bad),None,False)
    if len({r['id'] for r in cases})!=len(cases):raise ValueError('Duplicate corpus identity')
    return cases,inputs


def parse_output(raw):
    values={}
    for line in raw.decode('utf-8',errors='strict').splitlines():
        if not line.startswith('PEER_RESULT\t'):continue
        col=line.split('\t')
        if len(col)!=5 or col[1] in values:raise ValueError('Malformed/duplicate actual output')
        values[col[1]]={'status':col[2],'semantic':json.loads(col[3]),'hex':col[4]}
    return values


def compare(cases,go,rust):
    expected={row['id'] for row in cases}
    if len(expected)!=len(cases) or set(go)!=expected or set(rust)!=expected:raise ValueError('Peer case identity set differs')
    accepted=rejected=0
    for row in cases:
        for peer,actual in [('go',go[row['id']]),('rust',rust[row['id']])]:
            if actual['status']!=('accepted' if row['accepted'] else 'rejected'):raise ValueError(peer+' verdict differs: '+row['id'])
            if row['accepted']:
                canonical=lambda value:json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True)
                if canonical(actual['semantic'])!=canonical(row['semantic']):raise ValueError(peer+' semantic differs: '+row['id'])
                if actual['hex']!=row['hex']:raise ValueError(peer+' encode bytes differ: '+row['id'])
        if row['accepted']:accepted+=1
        else:rejected+=1
    return {'accepted':accepted,'rejected':rejected,'semantic_expectations':'actual_Java_fields_and_independent_sidecars_not_peer_decoders'}
