"""Collect complete, validated Java handler events; export alone is not runtime certification."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import struct

XID = '127.0.0.1:8091:1'
BRANCH_ID = 0x0102030405060708
TYPES = {'BEGIN':2, 'COMMIT':8, 'ROLLBACK':10, 'STATUS':16, 'REPORT':18,
         'REGISTER':12, 'BRANCH_REPORT':14, 'LOCK':22, 'REG_TM':102, 'REG_RM':104}
ERROR_NAMES = ['Unknown','BeginFailed','LockKeyConflict','IO','BranchRollbackFailed_Retriable',
 'BranchRollbackFailed_Unretriable','BranchRegisterFailed','BranchReportFailed','LockableCheckFailed',
 'BranchTransactionNotExist','GlobalTransactionNotExist','GlobalTransactionNotActive',
 'GlobalTransactionStatusInvalid','FailedToSendBranchCommitRequest','FailedToSendBranchRollbackRequest',
 'FailedToAddBranch','FailedLockGlobalTransaction','FailedWriteSession','FailedStore','NotRaftLeader',
 'LockKeyConflictFailFast','TransactionTimeout','CommitHeuristic','Broken','BranchPrepareFailed']


def expected_cases():
    """Independent specification of79 tested cases, not derived from captured payloads."""
    cases = {}
    initial = {'COMMIT':2, 'ROLLBACK':4, 'STATUS':0}
    success = {'COMMIT':9, 'ROLLBACK':11, 'STATUS':1, 'REPORT':4}
    tx = {'BEGIN':1, 'REGISTER':6, 'BRANCH_REPORT':7, 'LOCK':8}
    store = {'BEGIN':'begin global request failed. xid=null, msg=store failed',
             'COMMIT':f'global commit request failed. xid={XID}, msg=store failed',
             'ROLLBACK':f'global rollback request failed. xid={XID}, msg=store failed',
             'STATUS':f'global status request failed. xid={XID}, msg=store failed',
             'REGISTER':f'branch register request failed. xid={XID}, msg=store failed',
             'BRANCH_REPORT':f'branch report request failed. xid={XID}, branchId={BRANCH_ID}, msg=store failed',
             'LOCK':f'global lock query request failed. xid={XID}, msg=store failed'}

    def ordinary(op, failure, status=None):
        normal = failure == 'NONE'
        error = 0 if normal or failure == 'RUNTIME' or (op == 'REPORT' and failure == 'STORE') else tx.get(op,3) if failure == 'TRANSACTION' else 18
        msg = None if normal else 'TransactionException[core failed]' if failure == 'TRANSACTION' else \
              'RuntimeException[core failed]' if failure == 'RUNTIME' else \
              'RuntimeException[store failed]' if op == 'REPORT' else 'TransactionException['+store[op]+']'
        global_status = success.get(op,-1) if normal else 5 if op in initial else 4 if op == 'REPORT' else -1
        if status is not None: global_status = status
        return {'operation':op,'result_code':int(normal),'exception_code':error,'exception_name':ERROR_NAMES[error],
                'msg':msg,'global_status':global_status,'xid':XID if normal and op == 'BEGIN' else None,
                'branch_id':BRANCH_ID if normal and op == 'REGISTER' else 0,
                'lockable':normal and op == 'LOCK','version':None}

    for op in list(TYPES)[:8]:
        for failure in ['NONE','TRANSACTION','RUNTIME','STORE']:
            cases[f'matrix/{op}/{failure}'] = ordinary(op,failure)
    for op in initial:
        for failure in ['TRANSACTION','RUNTIME','STORE']:
            for lookup in ['ABSENT','THROWS','GETTER_THROWS','MUTATED_THEN_THROWS']:
                status = 15 if lookup == 'ABSENT' else 19 if lookup == 'MUTATED_THEN_THROWS' else initial[op]
                cases[f'fallback/{op}/{failure}/{lookup}'] = ordinary(op,failure,status)
    for code in [2,20]:
        row = ordinary('REGISTER','TRANSACTION')
        row.update(exception_code=code,exception_name=ERROR_NAMES[code],msg='TransactionException[lock denied]')
        cases['conflict/'+ERROR_NAMES[code]] = row
    row = ordinary('LOCK','NONE'); row['lockable'] = False
    cases['lock/success_false'] = row
    cases['report/null_failure'] = {'issue':'JAVA-V1-REPORT-NULL-001'}
    for role in ['TM','RM']:
        for behavior in ['ALLOW','DENY','THROW'] + (['DENY_CLOSED'] if role == 'RM' else []):
            cases[f'auth/{role}/{behavior}'] = {'operation':'REG_'+role,'result_code':int(behavior == 'ALLOW'),
                'exception_code':0,'exception_name':'Unknown','msg':None,'global_status':-1,'xid':None,
                'branch_id':0,'lockable':False,'version':'2.8.0-SNAPSHOT'}
    assert len(cases) == 79
    return cases


def encode_expected(fields):
    """Manual Java V1 oracle for this finite handler matrix; not a production codec."""
    def text(value):
        raw = (value or '').encode('utf-8')
        return struct.pack('>h',len(raw)) + raw
    op = fields['operation']
    out = struct.pack('>H',TYPES[op])+bytes([fields['result_code']])
    if op.startswith('REG_'): return out + text(fields['version'])
    if fields['result_code'] == 0: out += text(fields['msg'])
    out += bytes([fields['exception_code']])
    if op == 'BEGIN': out += text(fields['xid']) + text(None)
    elif op in ('COMMIT','ROLLBACK','STATUS','REPORT'): out += bytes([fields['global_status']])
    elif op == 'REGISTER': out += struct.pack('>q',fields['branch_id'])
    elif op == 'LOCK': out += struct.pack('>H',int(fields['lockable']))
    return out


def normalized(fields):
    def text(value): return (value or '').encode('utf-8').hex()
    return '\t'.join([fields['operation'],str(fields['result_code']),str(fields['exception_code']),
        text(fields['msg']),str(fields['global_status']),text(fields['xid']),str(fields['branch_id']),
        str(int(fields['lockable'])),text(fields['version'])]) + '\n'


def extract(source: Path, destination: Path):
    if source.stat().st_size > 32*1024*1024: raise ValueError('Java log exceeds capture bound')
    raw = source.read_bytes()
    log = raw.decode('utf-8-sig')
    if 'BUILD SUCCESS' not in log or 'Tests run: 135, Failures: 0, Errors: 0, Skipped: 0' not in log:
        raise ValueError('Successful full135-test Java run is required')
    specs, events = expected_cases(), {}
    for line in log.splitlines():
        if not line.startswith('SEATA_HANDLER_'): continue
        prefix, _, data = line.partition(' ')
        if prefix not in ('SEATA_HANDLER_WIRE','SEATA_HANDLER_UNENCODABLE'): raise ValueError('Unknown event kind')
        try: row = json.loads(data)
        except json.JSONDecodeError as error: raise ValueError('Malformed capture JSON') from error
        if not isinstance(row,dict): raise ValueError('Expected event object')
        case_id = row.get('case_id')
        if not isinstance(case_id,str) or case_id not in specs or case_id in events:
            raise ValueError('Unknown or duplicate handler case')
        if type(row.get('format_version')) is not int or row['format_version'] != 1:
            raise ValueError('Unsupported capture format')
        expected = specs[case_id]
        if 'issue' in expected:
            if prefix != 'SEATA_HANDLER_UNENCODABLE' or row != {'format_version':1,'case_id':case_id,**expected}:
                raise ValueError('Unencodable response must remain a separate diagnostic')
        else:
            if prefix != 'SEATA_HANDLER_WIRE' or set(row) != set(expected)|{'case_id','format_version','payload_hex'}:
                raise ValueError('Wrong wire event schema')
            for key,value in expected.items():
                if row[key] != value or type(row[key]) is not type(value):
                    raise ValueError(f'Independent semantic expectation mismatch: {case_id}/{key}')
            if not isinstance(row['payload_hex'],str) or not re.fullmatch(r'(?:[0-9a-f]{2})+',row['payload_hex']):
                raise ValueError('Malformed payload hex')
            if bytes.fromhex(row['payload_hex']) != encode_expected(expected):
                raise ValueError('Runtime bytes mismatch independent Java layout')
        events[case_id] = row
    if set(events) != set(specs): raise ValueError('Missing handler case events: '+str(len(specs)-len(events)))
    outputs, samples, unencodable = {}, [], []
    for case_id,row in sorted(events.items()):
        if 'issue' in row:
            unencodable.append({'case_id':case_id,'issue':row['issue']}); continue
        stem = case_id.replace('/','_').lower()
        payload = bytes.fromhex(row['payload_hex'])
        expected = normalized(specs[case_id]).encode('utf-8')
        outputs[stem+'.bin'] = payload
        outputs[stem+'.expect'] = expected
        samples.append({'case_id':case_id,'file':stem+'.bin','expectation':stem+'.expect','type_id':TYPES[row['operation']],
                        'bytes':len(payload),'sha256':hashlib.sha256(payload).hexdigest(),
                        'expectation_sha256':hashlib.sha256(expected).hexdigest()})
    manifest = {'format':'seata_v1_java_handler_runtime_v1','java_source':'cba73e180fd8896a99df6415b859d275122feaa7',
        'go_source':'82570d905919c1c66bb025a3a5979f25390de498','protocol_version':1,'serializer_id':1,'compressor_id':0,
        'source_log_sha256':hashlib.sha256(raw).hexdigest(),'case_count':len(events),'sample_count':len(samples),
        'unencodable_cases':unencodable,'samples':samples,
        'boundary':'Capture from a successful selected135 Java test run; export is not Go/Rust certification'}
    outputs['manifest.json'] = (json.dumps(manifest,indent=2)+'\n').encode('utf-8')
    outputs['events.json'] = (json.dumps([events[k] for k in sorted(events)],indent=2)+'\n').encode('utf-8')
    for name,data in outputs.items():
        path = destination/name
        if path.exists() and path.read_bytes() != data: raise ValueError('Refusing differing artifact overwrite: '+str(path))
    destination.mkdir(parents=True,exist_ok=True)
    for name,data in outputs.items():
        path = destination/name
        try:
            with path.open('xb') as stream: stream.write(data)
        except FileExistsError:
            if path.read_bytes() != data: raise ValueError('Artifact changed during capture: '+str(path))
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--log',required=True,type=Path)
    parser.add_argument('--destination',required=True,type=Path)
    options = parser.parse_args()
    try: result = extract(options.log,options.destination)
    except (ValueError,OSError) as error: parser.exit(1,str(error)+'\n')
    print(f"Captured {result['sample_count']} responses and {len(result['unencodable_cases'])} unencodable case")
