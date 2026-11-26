"""Read-only catalog/source and exact-counter verification, with evidence weakenings."""
import argparse,copy,datetime,json,re,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

EXPECT={
'completed':{'execute':{'execution_completed':4},'rollback':{'rollback_confirmed':1}},
'errors':{'execute':{'execution_completed':1,'execution_failed':3},'rollback':{'rollback_confirmed':1}},
'waits':{'execute':{'cancelled':1,'execution_failed':1},'rollback':{'rollback_unconfirmed':1}},
'delegates':{'prepare':{'prepared_unregistered':2},'check_commit':{'checked_without_witness':1},'rollback':{'rollback_confirmed':2}},
'rollback-cancelled':{'execute':{'execution_completed':1},'rollback':{'cancelled':1}},
'rollback-unconfirmed':{'execute':{'execution_completed':1},'rollback':{'rollback_unconfirmed':1}},
}
OUTCOMES={
'prepare':['prepared_unregistered','prepared_registered','registration_unknown','prepare_failed','cancelled'],
'check_commit':['checked_without_witness','checked_with_witness','check_failed','cancelled'],
'commit_no_change':['no_change_committed','no_change_commit_unknown','no_change_not_eligible','cancelled'],
'commit_changed':['changed_committed','changed_commit_unknown','changed_not_eligible','arming_failed','arming_uncertain','cancelled'],
'commit_and_report':['report_observed','report_local_failed','report_local_uncertain','report_claim_failed','report_unknown','report_failed','cancelled'],
'execute':['execution_completed','execution_failed','cancelled'],
'rollback':['rollback_confirmed','rollback_unconfirmed','cancelled'],
}
def catalog(folder,digest):
    need(sha(folder/'catalog.json')==digest,'Catalog digest differs')
    for row in json.loads((folder/'catalog.json').read_bytes()):
        member=(folder/row['path']).resolve();need(member.is_relative_to(folder.resolve()),'Catalog escapes root');need(sha(member)==row['sha256'],'Catalog member differs')
def verify(data):
    for name,wanted in EXPECT.items():
        observation=data[name];stages=observation['stages'];need(set(stages)==set(OUTCOMES),'Stage dimensions differ')
        for op,labels in OUTCOMES.items():
            stage=stages[op];need(type(stage['active']) is int and stage['active']==0,'Final active differs')
            need(set(stage['outcomes'])==set(labels),'Outcome dimensions differ')
            for label in labels:
                actual=stage['outcomes'][label];need(type(actual) is int and actual==wanted.get(op,{}).get(label,0),'Observed counter differs: '+name+'/'+op+'/'+label)
        if name!='delegates':need(type(observation['visible_value']) is int and observation['visible_value']==10,'Independent business value differs')
    need(type(data['waits']['real_row_waits']) is int and data['waits']['real_row_waits']>0,'Real MySQL row wait missing')
    for name in ['rollback-cancelled','rollback-unconfirmed']:
        need(type(data[name]['held_server_bytes']) is int and data[name]['held_server_bytes']>0,'Original server bytes not held')
    wanted={}
    for op,labels in OUTCOMES.items():
        wanted['seata_mysql_phase_one_active{operation="'+op+'"}']=0
        for label in labels:wanted['seata_mysql_phase_one_total{operation="'+op+'",outcome="'+label+'"}']=int((op,label) in [('prepare','prepare_failed'),('execute','execution_failed'),('rollback','rollback_confirmed')])
    for name in ['http-zero','http-after']:
        lines=data[name].splitlines();need(len(lines)==39,'HTTP series count differs');seen={}
        for line in lines:
            metric,value=line.rsplit(' ',1);need(value.isdecimal() and metric not in seen,'Invalid/duplicate metric');seen[metric]=int(value)
        need(set(seen)==set(wanted),'HTTP labels/names differ')
        need(seen==({k:0 for k in wanted} if name=='http-zero' else wanted),'HTTP counters differ')

def controls(data):
    labels=['missing_DML','cancel_as_success','failed_as_completed','duplicate_rollback','lost_rollback_as_confirmed','active_leak','boolean_counter','no_row_wait','no_held_bytes','HTTP_missing','HTTP_duplicate','HTTP_identity_label'];rejected=[]
    for label in labels:
        v=copy.deepcopy(data)
        if label=='missing_DML':v['completed']['stages']['execute']['outcomes']['execution_completed']=0
        elif label=='cancel_as_success':v['waits']['stages']['execute']['outcomes']['execution_completed']=1
        elif label=='failed_as_completed':v['errors']['stages']['execute']['outcomes']['execution_failed']=0
        elif label=='duplicate_rollback':v['delegates']['stages']['rollback']['outcomes']['rollback_confirmed']=4
        elif label=='lost_rollback_as_confirmed':v['rollback-unconfirmed']['stages']['rollback']['outcomes']['rollback_confirmed']=1
        elif label=='active_leak':v['completed']['stages']['execute']['active']=1
        elif label=='boolean_counter':v['completed']['stages']['execute']['outcomes']['cancelled']=False
        elif label=='no_row_wait':v['waits']['real_row_waits']=0
        elif label=='no_held_bytes':v['rollback-cancelled']['held_server_bytes']=0
        elif label=='HTTP_missing':v['http-after']='\n'.join(v['http-after'].splitlines()[:-1])
        elif label=='HTTP_duplicate':v['http-after']+=v['http-after'].splitlines()[0]+'\n'
        else:v['http-after']=v['http-after'].replace('operation="execute"','operation="user-sql"')
        try:verify(v)
        except (ValueError,KeyError):rejected.append(label)
        else:raise ValueError('Evidence weakening accepted: '+label)
    return rejected

def run(native,digest,mutations,mutation_digest):
    catalog(native,digest);catalog(mutations,mutation_digest);result={}
    commands=json.loads((native/'commands.json').read_bytes())
    for version in ['1.99','1.85']:
        for label,count in [('live',5),('http',1)]:
            need(next(c for c in commands if c['label']==version+'-'+label)['exit_code']==0,'Selected live command failed')
            need(('test result: ok. '+str(count)+' passed;').encode() in (native/(version+'-'+label+'.stdout')).read_bytes(),'Selected test count differs')
        folder=native/'observations'/version;data={name:json.loads((folder/(name+'.json')).read_bytes()) for name in EXPECT}
        data.update({'http-zero':(folder/'phase-one-http-zero.txt').read_text(encoding='utf-8'),'http-after':(folder/'phase-one-http-after-close.txt').read_text(encoding='utf-8')});verify(data);result[version]=dict(verified=True,rejected_controls=controls(data))
    for row in json.loads((native/'source-inputs.json').read_bytes()):need(sha(ROOT/row['path'])==row['sha256'],'Executed source differs')
    rows=json.loads((mutations/'mutations.json').read_bytes());need(len(rows)==4 and len({(r['Rust'],r['operation']) for r in rows})==4,'Four source mutations required')
    for row in rows:
        folder=ROOT/row['evidence'];catalog(folder,row['catalog_sha256']);commands=json.loads((folder/'commands.json').read_bytes());need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101,'Mutation did not fail')
        need(row['marker'].encode() in (folder/(row['Rust']+'-live.stderr')).read_bytes(),'Behavioral assertion absent');need(json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'],'Mutation runtime remains')
    need(json.loads((native/'cleanup.json').read_bytes())['owned_container_removed'],'Native runtime remains')
    out=native.parent/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir()
    save(out/'result.json',dict(status='BUSINESS_METRICS_AND_NEGATIVE_CONTROLS_VERIFIED',versions=result,source_mutations=4,independent_external_review='OPEN',WP07_complete=False))
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);p.add_argument('--native-sha',required=True);p.add_argument('--mutations',type=Path,required=True);p.add_argument('--mutations-sha',required=True);a=p.parse_args();run(a.native,a.native_sha,a.mutations,a.mutations_sha)
