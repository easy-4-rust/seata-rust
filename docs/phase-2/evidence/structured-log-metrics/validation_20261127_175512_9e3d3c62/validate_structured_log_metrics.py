"""Independently verify real authenticated log metrics and original TM request facts."""
import argparse
import copy
import datetime
import json
from pathlib import Path
import shutil
import uuid
from inventory_rebuilt import ROOT,need,save,sha,safe_path
from validate_structured_logs import catalog,sources,record

NAMES=['seata_structured_logs_accepted_total','seata_structured_logs_received_total']+['seata_structured_logs_dropped_total{reason="'+r+'"}' for r in ['full','closed','oversize','invalid']]


def parse(text):
    header,body=text.split('\r\n\r\n',1);need(header.startswith('HTTP/1.1 200 OK'),'Authorized scrape failed')
    values={}
    for line in body.splitlines():
        key,value=line.rsplit(' ',1);need(key not in values,'Duplicate metric series')
        need(value.isascii() and value.isdigit() and str(int(value))==value and int(value)<2**64,'Counter lost unsigned integer representation');values[key]=int(value)
    need(not any(s in text for s in ['private-log-metrics-token','wrong-private-token','private-xid','request_id','private-password','jdbc:mysql','resource_id']),'Private identity/token reached metrics')
    return values


def check(text,expected,tm=False):
    values=parse(text);logs={k:v for k,v in values.items() if k.startswith('seata_structured_logs_')};need(logs==dict(zip(NAMES,expected,strict=True)),'Six log counters/labels differ')
    other={k:v for k,v in values.items() if k not in logs};expected_tm={}
    if tm:
        for operation in ['begin','commit','rollback','status']:
            expected_tm['seata_tm_calls_active{operation="'+operation+'"}']=0
            expected_tm['seata_tm_protocol_errors_total{operation="'+operation+'"}']=0
            for outcome in ['observed','tc_rejected','unknown','not_sent','local_rejected','cancelled']:
                expected_tm['seata_tm_calls_total{operation="'+operation+'",outcome="'+outcome+'"}']=2 if operation=='status' and outcome=='observed' else 0
    need(other==expected_tm and len(values)==(38 if tm else 6),'Original fixed series changed')


def validate(folder):
    sources(folder);need(json.loads((folder/'commands.json').read_bytes())[0]['exit_code']==0,'Real target failed');need(b'test result: ok. 3 passed;' in (folder/'tests.stdout').read_bytes(),'Three real HTTP target summaries missing')
    obs=folder/'observations'
    def raw(name):return (obs/name).read_bytes().decode()
    for name,counts,combined in [('log-only-zero.http',[0,0,0,0,0,0],False),('log-only-queued.http',[1,0,1,0,1,1],False),('log-only-final.http',[1,1,1,1,1,1],False),('retained-final.http',[1,1,0,0,0,0],False),('tm-combined-before.http',[1,0,1,0,0,0],True),('tm-combined-final.http',[1,1,1,0,0,0],True)]:check(raw(name),counts,combined)
    unauthorized=raw('log-only-unauthorized.http');need(unauthorized.startswith('HTTP/1.1 401') and 'seata_' not in unauthorized and 'private' not in unauthorized,'Unauthorized response disclosed counters or token')
    proof=json.loads((obs/'tm-actual-facts.json').read_bytes());ids=proof['actual_request_ids'];need(len(ids)==2 and len(set(ids))==2 and all(type(v) is int and -(2**31)<=v<2**31 for v in ids),'Actual TM request facts missing')
    event=proof['first_original_event'];record(event);need(event['fields']['request_id']==str(ids[0]) and event['fields']['operation']=='Status' and event['fields']['outcome']=='response' and event['fields']['xid']=='Some("tc:tm-metrics-private-xid")','Received JSON differs from original first request')
    need(type(proof['actual_observed_calls']) is int and proof['actual_observed_calls']==2 and proof['one_received_one_dropped'] is True,'Actual TM success/overflow facts differ')
    rejected=[]
    final=raw('log-only-final.http');before=raw('tm-combined-before.http')
    variants=[('missing-series',final.replace('seata_structured_logs_received_total 1\n',''),False,[1,1,1,1,1,1]),('duplicate-series',final+'seata_structured_logs_received_total 1\n',False,[1,1,1,1,1,1]),('wrong-counter',final.replace('reason="full"} 1','reason="full"} 0'),False,[1,1,1,1,1,1]),('extra-label',final.replace('reason="full"','reason="full",xid="private-xid"'),False,[1,1,1,1,1,1]),('false-durable-stage',before.replace('seata_structured_logs_received_total 0','seata_structured_logs_received_total 1'),True,[1,0,1,0,0,0]),('wrong-original-tm',before.replace('outcome="observed"} 2','outcome="observed"} 3'),True,[1,0,1,0,0,0]),('noninteger-counter',final.replace('received_total 1','received_total 1.0'),False,[1,1,1,1,1,1]),('overflow-counter',final.replace('received_total 1','received_total 18446744073709551616'),False,[1,1,1,1,1,1])]
    for label,text,combined,counts in variants:
        try:check(text,counts,combined)
        except (ValueError,AssertionError):rejected.append(label)
        else:raise ValueError('Evidence weakening accepted: '+label)
    return rejected


def mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes());need(len(rows)==4 and {(r['Rust'],r['operation']) for r in rows}=={(v,k) for v in ['1.99','1.85'] for k in ['omit_export','wrong_full_counter']},'Actual mutations incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256']);need(json.loads((child/'commands.json').read_bytes())[0]['exit_code']==101,'Mutation target did not fail')
        need(row['marker'].encode() in (child/'tests.stdout').read_bytes()+(child/'tests.stderr').read_bytes(),'Mutation failed for unrelated reason')
    for row in json.loads((folder/'restored.json').read_bytes()):need(row['exact_original_restored'] is True and sha(safe_path(ROOT,row['path']))==row['sha256'],'Original production bytes not restored')


def run(args):
    folders={k:ROOT/getattr(args,k) for k in ['rust199','rust185','mutations']}
    for k,folder in folders.items():catalog(folder,getattr(args,k+'_sha'))
    rejected={k:validate(folders[k]) for k in ['rust199','rust185']};mutations(folders['mutations'])
    out=ROOT/'docs/phase-2/evidence/structured-log-metrics'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='AUTHENTICATED_LOG_METRICS_AND_ORIGINAL_TM_FACTS_VALIDATED',authority={k:dict(path=getattr(args,k),catalog_sha256=getattr(args,k+'_sha')) for k in folders},rejected_controls=rejected,source_mutations=4,final_source_inputs_match_current_bytes=True,external_review='OPEN',WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['rust199','rust185','mutations']:parser.add_argument('--'+key,required=True);parser.add_argument('--'+key+'-sha',required=True)
    run(parser.parse_args())
