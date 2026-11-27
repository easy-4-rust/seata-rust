"""Verify actual application lifecycle, independent sinks, requests and pinned dependencies."""
import argparse,copy,datetime,hashlib,json,subprocess,tarfile,tomllib,uuid,shutil
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha,safe_path
from validate_structured_logs import catalog,sources,record,mysql

BASE='636914a58fc2ff9adda84dba1dbb6e869df6a739'
MESSAGES=['tm_rpc_outcome','rm_rpc_outcome','rm_callback_decision','rm_callback_response_write','mysql_control_outcome','mysql_undo_outcome','mysql_undo_checksum_rejected']

def own(event):
    record(event);text=json.dumps(event)
    need(not any(s in text for s in ['application-private-payload','private-span-password','private-tm-span','private-disabled-span']),'Application private span/event value reached Seata JSON')

def dynamic(value):
    need(type(value['baseline_disabled_span']) is bool and value['baseline_disabled_span']==value['disabled_span'],'Original Registry admission changed')
    need(value['opened']==[] and len(value['application_events'])==1,'Dynamic application filter changed output')
    event=value['application_events'][0];need(event[0]=='app::composition' and event[1] is None and 'application-private-payload' in event[2]['payload'],'Application event payload/context lost')
    own(value['seata_event']);need(value['accepted']==1 and value['seata_event']['fields']['request_id']=='-17','Independent Seata event lost')

def lifecycle(value):
    root=int(value['root_id']);child=int(value['child_id']);need(root!=child and root>0 and child>0,'Original span identities differ')
    need(value['parents'][str(root)] is None and type(value['parents'][str(child)]) is int and value['parents'][str(child)]==root,'Original parent/child edge changed')
    need(all(type(v) is int for v in value['opened']+value['closed']),'Span ID representation changed')
    need(value['opened']==[root,child] and value['closed']==[child,root],'Span creation/final close changed')
    need(value['recorded']==1 and value['entered']==value['exited'] and value['entered']>=2 and value['follows']==[[child,root]],'Record/follows/enter/exit lifecycle changed')
    need(value['span_fields'][str(root)]['password']=='"private-span-password"' and value['span_fields'][str(root)]['progress']=='7','Application original span fields lost')
    need(len(value['events'])==2 and all(e[1]==child for e in value['events']),'Application event context changed')
    need('application-private-payload' in value['events'][0][2]['payload'],'Original application payload filtered out')

def templates(value):
    need(len(value['records'])==7 and [e['fields']['message'] for e in value['records']]==MESSAGES,'Missing/duplicate composed template')
    for event in value['records']:own(event)
    need(type(value['application_event_count']) is int and type(value['accepted']) is int and value['application_event_count']==1 and value['accepted']==7,'Sibling app output or own event count changed')

def actual_tm(value):
    event=value['event'];own(event);ids=value['actual_request_ids'];need(len(ids)==2 and len(set(ids))==2 and all(type(v) is int and -(2**31)<=v<2**31 for v in ids),'Actual original TM requests missing')
    f=event['fields'];need(set(f)=={'operation','xid','request_id','outcome','message'} and f['request_id']==str(ids[0]) and f['xid']=='Some("tc:layer-native")' and f['outcome']=='response','Original first TM event changed/enriched')
    need(all(type(value[k]) is int for k in ['accepted','received','dropped_full']) and (value['accepted'],value['received'],value['dropped_full'])==(1,1,1),'Shared original queue budget/counts changed')
    root=int(value['application_span_id']);app=value['application_state'];need(app['opened']==app['closed']==[root] and app['entered']==app['exited'] and app['entered']>0,'Instrumented application span did not close once')
    need(len(app['events'])==1 and app['events'][0][1]==root and 'application-private-payload' in app['events'][0][2]['payload'],'Application instrumented future context lost')

def native(folder):
    sources(folder);need(json.loads((folder/'commands.json').read_bytes())[0]['exit_code']==0 and b'test result: ok. 5 passed;' in (folder/'tests.stdout').read_bytes(),'Five final native targets not executed')
    obs=folder/'observations';values={}
    for name,check in [('dynamic-application',dynamic),('application-lifecycle',lifecycle),('seven-composed-templates',templates),('actual-tm-composition',actual_tm)]:
        value=json.loads((obs/(name+'.json')).read_bytes());check(value);values[name]=value
    global_event=json.loads((obs/'global-filter.json').read_bytes());own(global_event);need(global_event['level']=='WARN' and global_event['fields']['request_id']=='-18','Application global filter bypassed')
    rejected=[]
    def reject(label,check,value):
        try:check(value)
        except (ValueError,AssertionError,KeyError,TypeError):rejected.append(label)
        else:raise ValueError('Weakened source evidence accepted: '+label)
    v=copy.deepcopy(values['dynamic-application']);v['disabled_span']=not v['baseline_disabled_span'];reject('changed-registry-admission',dynamic,v)
    v=copy.deepcopy(values['dynamic-application']);v['application_events']=[];reject('lost-app-event',dynamic,v)
    for key,replacement in [('closed',[]),('recorded',0),('follows',[])]:
        v=copy.deepcopy(values['application-lifecycle']);v[key]=replacement;reject('lifecycle-'+key,lifecycle,v)
    v=copy.deepcopy(values['seven-composed-templates']);v['records']=v['records'][:-1];reject('missing-template',templates,v)
    v=copy.deepcopy(values['seven-composed-templates']);v['records']+=[v['records'][0]];reject('duplicate-template',templates,v)
    v=copy.deepcopy(values['seven-composed-templates']);v['records'][0]['fields']['password']='private-span-password';reject('span-field-leak',templates,v)
    for key,replacement in [('request_id','0'),('password','private-tm-span')]:
        v=copy.deepcopy(values['actual-tm-composition']);v['event']['fields'][key]=replacement;reject('tm-'+key,actual_tm,v)
    v=copy.deepcopy(values['actual-tm-composition']);v['dropped_full']=0;reject('false-queue-retention',actual_tm,v)
    v=copy.deepcopy(values['actual-tm-composition']);v['application_state']['closed']=[];reject('lost-instrumented-close',actual_tm,v)
    v=copy.deepcopy(values['actual-tm-composition']);v['accepted']=True;reject('boolean-counter',actual_tm,v)
    return rejected

def mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes());need(len(rows)==6 and {(r['Rust'],r['operation']) for r in rows}=={(v,k) for v in ['1.99','1.85'] for k in ['global_filter','omit_forward','duplicate_forward']},'Source mutations incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256']);need(json.loads((child/'commands.json').read_bytes())[0]['exit_code']==101,'Mutation did not fail real target');need(row['marker'].encode() in (child/'tests.stdout').read_bytes()+(child/'tests.stderr').read_bytes(),'Unrelated mutation failure')
    for row in json.loads((folder/'restored.json').read_bytes()):need(row['exact_original_restored'] is True and sha(safe_path(ROOT,row['path']))==row['sha256'],'Production source not restored exactly')

def dependencies(folder):
    old=tomllib.loads(subprocess.run(['git','show',BASE+':Cargo.lock'],cwd=ROOT,capture_output=True,check=True).stdout.decode());new=tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'))
    previous={(p['name'],p['version']):p for p in old['package']};current={(p['name'],p['version']):p for p in new['package']};expected={('tracing-subscriber','0.3.23'),('sharded-slab','0.1.7'),('thread_local','1.1.10')};need(set(current)-set(previous)==expected and set(previous)<=set(current),'Unexpected dependency version change')
    for key,value in previous.items():
        wanted=copy.deepcopy(value)
        if key[0]=='seata-observability':wanted['dependencies'].append('tracing-subscriber')
        need(current[key]==wanted,'Unrelated original dependency changed')
    rows=json.loads((folder/'verified-inputs.json').read_bytes());need({(r['name'],r['version']) for r in rows}==expected,'Dependency archive incomplete')
    for row in rows:
        need(sha(folder/row['crate_file'])==row['registry_checksum']==current[(row['name'],row['version'])]['checksum'],'Published dependency checksum differs')
        with tarfile.open(folder/row['crate_file'],'r:gz') as archive:
            prefix=row['name']+'-'+row['version']+'/';files={m.name[len(prefix):]:m for m in archive.getmembers() if m.isfile() and m.name.startswith(prefix)};need(set(files)==set(row['extracted_file_hashes']),'Packaged files differ')
            for name,member in files.items():need(hashlib.sha256(archive.extractfile(member).read()).hexdigest()==row['extracted_file_hashes'][name],'Packaged source hash differs')

def run(args):
    folders={k:ROOT/getattr(args,k) for k in ['rust199','rust185','mysql','mutations','dependencies']}
    for key,folder in folders.items():catalog(folder,getattr(args,key+'_sha'))
    rejected={k:native(folders[k]) for k in ['rust199','rust185']};mysql(folders['mysql']);mutations(folders['mutations']);dependencies(folders['dependencies'])
    for version in ['1.99','1.85']:
        proof=json.loads((folders['mysql']/'observations'/version/'case-0/mysql-native-proof.json').read_bytes());need(proof['source']['layer_mode'] is True,'Real MySQL fixture did not use new Layer')
    out=ROOT/'docs/phase-2/evidence/structured-log-layer'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='LAYER_COMPOSITION_AND_ORIGINAL_SOURCE_FACTS_VALIDATED',authority={k:dict(path=getattr(args,k),catalog_sha256=getattr(args,k+'_sha')) for k in folders},rejected_controls=rejected,source_mutations=6,final_source_inputs_match_current_bytes=True,external_review='OPEN',original_java_tc_rerun=False,WP07_complete=False));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['rust199','rust185','mysql','mutations','dependencies']:parser.add_argument('--'+key,required=True);parser.add_argument('--'+key+'-sha',required=True)
    run(parser.parse_args())
