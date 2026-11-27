"""Read-only independent checks of bounded JSON exports and actual source-side facts."""
import argparse
import copy
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile
from inventory_rebuilt import ROOT, need, save, sha, safe_path
from mysql_undo_executor_live import IMAGE

PIN='cba73e180fd8896a99df6415b859d275122feaa7'
BASE='2c8588de8bbaeaf5af766da30e76afcb9cc5220c'
SCHEMAS={
    'tm_rpc_outcome':('seata_tm::rpc',{'operation','xid','request_id','outcome'}),
    'rm_rpc_outcome':('seata_rm::rpc',{'operation','xid','branch_id','resource_sha256','request_id','outcome'}),
    'rm_callback_decision':('seata_rm::callback',{'request_id','connection_generation','xid','branch_id','resource_sha256','phase','branch_status','result_code'}),
    'rm_callback_response_write':('seata_rm::callback_write',{'request_id','connection_generation','response_count','outcome'}),
    'mysql_control_outcome':('seata_db_mysql::control',{'operation','requested_phase','xid','branch_id','resource_sha256','outcome','resolution','commit_attempted'}),
    'mysql_undo_outcome':('seata_db_mysql::undo',{'operation','xid','branch_id','resource_sha256','outcome','observation','commit_attempted'}),
    'mysql_undo_checksum_rejected':('seata_db_mysql::undo',{'xid','branch_id','resource_digest','outcome'}),
}


def catalog(folder,digest):
    need(sha(folder/'catalog.json')==digest,'Trusted catalog differs')
    rows=json.loads((folder/'catalog.json').read_bytes())
    need(len(rows)==len({r['path'] for r in rows}),'Duplicate catalog member')
    need({r['path'] for r in rows}=={p.relative_to(folder).as_posix() for p in folder.rglob('*') if p.is_file() and p.name!='catalog.json'},'Uncatalogued file')
    for row in rows:need(sha(safe_path(folder,row['path']))==row['sha256'],'Catalog byte mismatch')


def sources(folder):
    rows=json.loads((folder/'source-inputs.json').read_bytes())
    with zipfile.ZipFile(folder/'source-inputs.zip') as archive:
        need(set(archive.namelist())=={r['path'] for r in rows},'Source archive member mismatch')
        for row in rows:need(hashlib.sha256(archive.read(row['path'])).hexdigest()==row['sha256']==sha(safe_path(ROOT,row['path'])),'Final executed source drift')


def record(event):
    need(set(event)=={'target','level','timestamp_unix_millis','fields'},'Unknown JSON envelope field')
    need(type(event['timestamp_unix_millis']) is int and 0<event['timestamp_unix_millis']<2**64,'Invalid timestamp')
    need(event['level'] in ['INFO','WARN'],'Unsupported source level')
    fields=event['fields'];need(type(fields) is dict and all(type(v) is str for v in fields.values()),'Fields changed original textual representation')
    message=fields.get('message');need(message in SCHEMAS,'Unknown fixed message')
    target,names=SCHEMAS[message]
    need(event['target']==target and set(fields)==names|{'message'},'Unexpected event target/field set')
    text=json.dumps(event)
    need(not any(s in text for s in ['private-password','private-lock-key','private-application-data','private-callback-data','private-mysql-application-data','private-tc-response-credential','private-rm-tc-error','jdbc:mysql','mysql://exact-resource','seata-mysql-fixture-only','INSERT INTO']),'Private payload or resource text reached export')


def lines(path):
    raw=path.read_bytes();need(raw.endswith(b'\n'),'Incomplete last JSON line')
    records=[json.loads(line) for line in raw.splitlines()]
    for line,event in zip(raw.splitlines(keepends=True),records,strict=True):need(len(line)<=4096 and line.count(b'\n')==1,'Record byte/line budget differs');record(event)
    return records


def stats(proof,count):
    need(proof['file_write_all_sync_all_and_readback_confirmed'] is True,'Consumer durable-write/readback proof missing')
    values=proof['stats'];need(set(values)=={'accepted','received','dropped_full','dropped_closed','dropped_oversize','dropped_invalid'} and all(type(v) is int and v>=0 for v in values.values()),'Invalid source stats')
    need(values['accepted']==values['received']==count and not any(v for k,v in values.items() if k.startswith('dropped_')),'Source delivery counts differ')


def request_id(value):
    if value.startswith('Some(') and value.endswith(')'):value=value[5:-1]
    need(value==str(int(value)) and -(2**31)<=int(value)<2**31,'Invalid signed request correlation')
    return int(value)


def digest(value,resource):
    if value.startswith('Some(') and value.endswith(')'):value=json.loads(value[5:-1])
    need(value==hashlib.sha256(resource.encode()).hexdigest(),'Resource digest correlation differs')


def tm(records,proof):
    need(len(records)==3,'Missing/duplicate TM record');stats(proof,3)
    source=proof['source'];need(source['xid']=='tc:log-native' and len(set(source['actual_request_ids']))==3,'Source TM request facts differ')
    for index,event in enumerate(records):
        record(event);f=event['fields'];need(event['target']=='seata_tm::rpc' and f['operation']=='Status' and f['xid']=='Some("tc:log-native")','TM original identity changed')
        need(request_id(f['request_id'])==source['actual_request_ids'][index],'TM request ID changed')
        need(f['outcome']==('unknown' if index==2 else 'response'),'Rejected/lost TM response misclassified')


def rm(records,proof):
    need(len(records)==4,'Missing/duplicate RM record');stats(proof,4);source=proof['source']
    need(len(set(source['actual_request_ids']))==3 and source['xid']=='tc:log-rm','Source RM request facts differ')
    for index,event in enumerate(records):
        record(event);f=event['fields'];need(event['target']=='seata_rm::rpc' and f['operation']=='LockQuery' and f['xid']=='Some("tc:log-rm")' and f['branch_id']=='None','RM original request changed')
        if index<3:need(request_id(f['request_id'])==source['actual_request_ids'][index],'RM request ID changed')
        else:need(f['request_id']=='None','Not-sent call invented request ID')
        need(f['outcome']==(['response','response','unknown','not_sent'][index]),'RM outcome changed');digest(f['resource_sha256'],source['resource'])


def callback(records,proof):
    need(len(records)==6,'Missing/duplicate callback record');stats(proof,6);source=proof['source']
    need(source['actual_effects']==1 and type(source['actual_effects']) is int and source['branch_id']=='9007199254740995','Callback effect/large identity changed')
    need([(r['id'],r['branch_status_code']) for r in source['responses']]==[(-2**31,5),(-11,5),(-12,10)],'Original response facts differ')
    for index,response in enumerate(source['responses']):
        pair=[e for e in records if request_id(e['fields']['request_id'])==response['id']]
        need(len(pair)==2,'Callback decision/write pair differs')
        decision,write=pair;record(decision);record(write);f=decision['fields'];w=write['fields']
        need(decision['target']=='seata_rm::callback' and write['target']=='seata_rm::callback_write','Callback event order differs')
        need(f['xid']=='tc:log-callback' and f['branch_id']==source['branch_id'],'Large branch identity lost precision')
        need(f['phase']==('Rollback' if index==2 else 'Commit') and f['branch_status']==('PhaseTwoRollbackFailedUnretryable' if index==2 else 'PhaseTwoCommitted') and f['result_code']==('Failed' if index==2 else 'Success'),'Local decision differs from original response')
        need(f['connection_generation']==w['connection_generation'] and int(f['connection_generation'])>0,'Original callback generation changed')
        need(w['outcome']=='local_written' and w['response_count']=='1','Local write falsely presented as TC delivery');digest(f['resource_sha256'],source['resource'])


def native(folder):
    sources(folder);need(json.loads((folder/'commands.json').read_bytes())[0]['exit_code']==0,'Rust targets failed')
    need(b'test result: ok. 7 passed;' in (folder/'tests.stdout').read_bytes() and b'test result: ok. 2 passed;' in (folder/'tests.stdout').read_bytes() and b'test result: ok. 1 passed;' in (folder/'tests.stdout').read_bytes(),'Expected target summaries absent')
    obs=folder/'observations';manual=json.loads((obs/'seven-templates.json').read_bytes());need([v['fields']['message'] for v in manual]==list(SCHEMAS),'Seven template routing incomplete')
    for event in manual:record(event)
    need(manual[0]['fields']['xid']=='tc:quote"line\n' and manual[0]['fields']['request_id']=='-7' and manual[1]['fields']['request_id']=='Some(-17)','Textual/escaped identity changed')
    values={}
    for name,check in [('tm-native',tm),('rm-native',rm),('rm-callback',callback)]:
        records=lines(obs/(name+'.ndjson'));proof=json.loads((obs/(name+'-proof.json')).read_bytes());check(records,proof);values[name]=(records,proof)
    return values


def weaken(values):
    rejected=[]
    def reject(label,check,records,proof):
        try:check(records,proof)
        except (ValueError,AssertionError,KeyError,TypeError):rejected.append(label)
        else:raise ValueError('Weakened JSON/source evidence accepted: '+label)
    records,proof=values['tm-native']
    reject('missing-tm',tm,records[:-1],proof);reject('duplicate-tm',tm,records+[records[0]],proof)
    for key,value in [('request_id','1'),('xid','Some("other")'),('outcome','response')]:
        changed=copy.deepcopy(records);changed[2]['fields'][key]=value;reject('wrong-tm-'+key,tm,changed,proof)
    changed=copy.deepcopy(proof);changed['stats']['received']=True;reject('boolean-counter',tm,records,changed)
    records,proof=values['rm-native']
    for key,value in [('resource_sha256','0'*64),('resource_sha256',proof['source']['resource']),('sql','INSERT INTO private')]:
        changed=copy.deepcopy(records);changed[0]['fields'][key]=value;reject('wrong-rm-'+key+'-'+value[:4],rm,changed,proof)
    records,proof=values['rm-callback']
    for index,key,value in [(0,'branch_id','9007199254740996'),(0,'branch_id',9007199254740995),(1,'connection_generation','9999'),(1,'outcome','tc_received'),(0,'branch_status','PhaseTwoRollbacked')]:
        changed=copy.deepcopy(records);changed[index]['fields'][key]=value;reject('wrong-callback-'+key+'-'+str(value),callback,changed,proof)
    changed=copy.deepcopy(proof);changed['source']['actual_effects']=2;reject('false-effect-total',callback,records,changed)
    return rejected


def mysql(folder):
    sources(folder);commands=json.loads((folder/'commands.json').read_bytes());need(all(c['exit_code']==0 for c in commands if c['label']!='confirm-removed'),'MySQL command failed')
    need(next(c for c in commands if c['label']=='confirm-removed')['exit_code']!=0,'Owned MySQL still present')
    state=json.loads((folder/'inspect.stdout').read_bytes())[0];need(state['Id']==(folder/'create.stdout').read_text().strip() and state['Image']==IMAGE and len(state['Config']['Labels']['seata.structured.logs.owner'])==32,'Owned MySQL identity changed')
    need(state['HostConfig']['PortBindings']['3306/tcp'][0]['HostIp']=='127.0.0.1' and json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'] is True,'MySQL cleanup/bind differs')
    original=subprocess.run(['git','show',PIN+':script/client/at/db/mysql.sql'],cwd=ROOT.parent/'seata-java-phase09-oracle',capture_output=True,check=True).stdout
    need((folder/'original-undo.sql').read_bytes()==original,'Java Undo DDL differs from fixed Git blob')
    for version in ['1.99','1.85']:
        need((folder/(version+'-durable-fence.stdout')).read_bytes().strip()==b'4242\t1','Independent SQL durable fence missing')
        obs=folder/'observations'/version/'case-0';records=lines(obs/'mysql-native.ndjson');proof=json.loads((obs/'mysql-native-proof.json').read_bytes());stats(proof,3);need(len(records)==3,'MySQL event count differs');source=proof['source']
        for index,event in enumerate(records):
            f=event['fields'];need(f['xid']=='logs:fixture' and f['branch_id']=='4242','MySQL original identity differs');digest(f['resource_sha256'],source['resource_id'])
            need(f['commit_attempted']==('true' if index==0 else 'false'),'MySQL actual COMMIT boundary differs')
            if index<2:need(f['message']=='mysql_undo_outcome' and f['outcome']=='completed' and f['observation']==('fenced_inserted' if index==0 else 'fenced_existing'),'Original Undo observation differs')
            else:need(f['message']=='mysql_control_outcome' and f['operation']=='bind_rollback' and f['requested_phase']=='rollback' and f['outcome']=='before_commit_error' and f['resolution']=='not_observed' and source['closed_pool_recovery']=='Unknown','Public Unknown/control error distinction lost')


def mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes());expected={(v,k) for v in ['1.99','1.85'] for k in ['pre_format_schema','nonblocking_queue','encoded_byte_budget','resource_digest']}
    need(len(rows)==8 and {(r['Rust'],r['operation']) for r in rows}==expected,'Real source counterexamples incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256']);need(json.loads((child/'commands.json').read_bytes())[0]['exit_code']==101,'Mutation did not fail target')
        need(row['marker'].encode() in (child/'tests.stdout').read_bytes()+(child/'tests.stderr').read_bytes(),'Mutation failed for unrelated reason')
    for row in json.loads((folder/'restored.json').read_bytes()):need(row['exact_original_restored'] is True and sha(safe_path(ROOT,row['path']))==row['sha256'],'Mutated source not restored')


def run(args):
    folders={key:ROOT/getattr(args,key) for key in ['rust199','rust185','mysql','mutations']}
    for key,folder in folders.items():catalog(folder,getattr(args,key+'_sha'))
    rejected={key:weaken(native(folders[key])) for key in ['rust199','rust185']};mysql(folders['mysql']);mutations(folders['mutations'])
    baseline=subprocess.run(['git','show',BASE+':Cargo.lock'],cwd=ROOT,capture_output=True,check=True).stdout
    start=baseline.index(b'name = "seata-observability"');end=baseline.index(b'[[package]]',start);part=baseline[start:end].replace(b' "thiserror",',b' "serde_json",\n "thiserror",').replace(b' "tokio",',b' "tokio",\n "tracing",')
    need((ROOT/'Cargo.lock').read_bytes()==baseline[:start]+part+baseline[end:],'Unrelated lockfile change')
    out=ROOT/'docs/phase-2/evidence/structured-logs'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='STRUCTURED_LOG_EXPORTS_AND_SOURCE_FACTS_VERIFIED',authority={k:dict(path=getattr(args,k),catalog_sha256=getattr(args,k+'_sha')) for k in folders},rejected_controls=rejected,source_mutations=8,final_source_inputs_match_current_bytes=True,lockfile_only_adds_optional_observability_edges=True,original_java_tc_rerun=False,external_review='OPEN',WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['rust199','rust185','mysql','mutations']:parser.add_argument('--'+key,required=True);parser.add_argument('--'+key+'-sha',required=True)
    run(parser.parse_args())
