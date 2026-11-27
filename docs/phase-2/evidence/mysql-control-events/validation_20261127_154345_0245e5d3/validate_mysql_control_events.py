"""Independent control-event fields, original MySQL packets and TC callback linkage."""
import argparse,copy,datetime,hashlib,json,re,shutil,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
from qualify_rebuilt import verify_catalog
from mysql_undo_executor_live import IMAGE
from validate_rm_callback_write_events import wire
FIELDS={'target','message','operation','requested_phase','xid','branch_id','resource_sha256','outcome','resolution','commit_attempted'}
OPS=['enroll','observe','bind_commit','bind_rollback','commit','rollback']

def event(row,xid,branch,resource):
    need(set(row)==FIELDS and all(type(v) is str for v in row.values()),'Unexpected payload or field type')
    need(row['target']=='seata_db_mysql::control' and row['message']=='mysql_control_outcome','Wrong control target/message')
    need(row['xid']==xid and row['branch_id']==str(branch) and row['resource_sha256']==hashlib.sha256(resource.encode()).hexdigest(),'Original correlation/digest differs')
    op=row['operation'];need(op in OPS,'Unknown control operation')
    phase='none' if op=='enroll' else 'commit' if op in ['commit','bind_commit'] else 'rollback' if op in ['rollback','bind_rollback'] else row['requested_phase']
    need(row['requested_phase']==phase and phase in ['none','commit','rollback'] and (op!='observe' or phase!='none'),'Requested phase differs')
    need(row['outcome'] in ['completed','unknown_commit','before_commit_error','cancelled'],'Outcome differs')
    need(row['resolution'] in ['applied','unresolved','not_observed'] and row['commit_attempted'] in ['true','false'],'Resolution/attempt marker differs')
    if row['outcome']!='completed':need(row['resolution']=='not_observed','Unconfirmed result promoted to observed state')
    if row['outcome']=='unknown_commit':need(row['commit_attempted']=='true','Unknown COMMIT lost attempt boundary')
    if row['outcome']=='before_commit_error':need(row['commit_attempted']=='false','Pre-COMMIT error gained attempt boundary')
    text=json.dumps(row)
    for secret in ['jdbc:mysql','event-secret-value','seata-mysql-fixture-only','application-data-private','private-password','private-token','driver-error-private','INSERT INTO']:need(secret not in text,'Sensitive input leaked')

def packet(text,request):
    raw=bytes.fromhex(text);need(raw.hex()==text and len(raw)>=4 and len(raw)==4+int.from_bytes(raw[:3],'little'),'Invalid original MySQL frame')
    body=raw[4:]
    if request:need(raw[3]==0 and body==b'\x03COMMIT','Not the original COMMIT command')
    else:need(raw[3]==1 and len(body)>=7 and body[:3]==b'\x00\x00\x00' and int.from_bytes(body[3:5],'little')&1==0,'Not original successful COMMIT OK outside a transaction')

def native(root):
    state,=json.loads((root/'inspect.stdout').read_bytes());need(state['Id']==(root/'create.stdout').read_text(encoding='utf-8').strip() and state['Image']==IMAGE,'Owned native database identity differs')
    need(re.fullmatch('[a-f0-9]{32}',state['Config']['Labels']['seata.control.events.owner']) is not None,'Native owner nonce differs')
    environment=dict(item.split('=',1) for item in state['Config']['Env']);binding,=state['HostConfig']['PortBindings']['3306/tcp'];need(binding['HostIp']=='127.0.0.1','Native database is not loopback')
    runtime=dict(database=environment['MYSQL_DATABASE'],port=int(binding['HostPort']));info={} 
    expected=[('enroll','completed','unresolved','true'),('bind_rollback','completed','unresolved','true'),('rollback','completed','applied','true'),('bind_rollback','completed','applied','false'),('rollback','completed','applied','false')]
    for version in ['1.99','1.85']:
        folder=root/'observations'/version
        def rows(case,name):
            path=folder/case;contexts={}
            for p in path.glob('control-event-context-*.json'):
                value=json.loads(p.read_bytes());digest=hashlib.sha256(value['resource_id'].encode()).hexdigest();need(p.name=='control-event-context-'+digest+'.json' and value['xid']=='rollback:xid' and value['branch_id']==42,'Native identity metadata differs');contexts[digest]=value
            value=json.loads((path/name).read_bytes());events=value['events'] if type(value) is dict else value
            for row in events:
                context=contexts[row['resource_sha256']];event(row,context['xid'],context['branch_id'],context['resource_id'])
                need(re.fullmatch(r'jdbc:mysql://127\.0\.0\.1:[0-9]+/'+re.escape(runtime['database']),context['resource_id']) is not None,'Native resource points outside owned database')
            return value,events
        _,success=rows('case-0','control-events-success.json');need([(e['operation'],e['outcome'],e['resolution'],e['commit_attempted']) for e in success]==expected,'Durable write and duplicate-read boundaries differ')
        _,before=rows('case-1','control-events-before-commit.json');need(len(before)==2 and [e['outcome'] for e in before]==['cancelled','before_commit_error'] and all(e['commit_attempted']=='false' for e in before),'Cancellation before COMMIT differs')
        for name,outcome in [('unknown','unknown_commit'),('cancelled','cancelled')]:
            value,events=rows('case-2','control-events-'+name+'-commit.json');need(len(events)==3 and events[0]['operation']=='enroll' and events[0]['outcome']==outcome and events[0]['commit_attempted']=='true','Held control COMMIT classification differs')
            need(events[1]['operation']=='bind_rollback' and events[2]['operation']=='rollback' and events[2]['outcome']=='completed' and events[2]['resolution']=='applied' and all(e['commit_attempted']=='true' for e in events[1:]),'Same original branch did not recover')
            need(value['before_recovery']==[1,0,0] and value['after_recovery']==[1,2,1] and type(value['business_rows']) is int and value['business_rows']==0,'Observed durable database state differs');packet(value['actual_commit_packet'],True);packet(value['actual_ok_packet'],False)
        info[version]=dict(verified=True,selected_tests=3,original_COMMIT_OK_faults=2)
    return info

def java(data,scenario):
    rows=data['events'];observed=data['observation'];phase='rollback' if scenario.startswith('rollback') else 'commit'
    need(rows,'No original Java control events')
    for row in rows:
        event(row,observed['xid'],observed['branch_id'],observed['resource_id'])
        need(row['outcome']=='completed','Selected original Java callback path unexpectedly failed')
        need(row['requested_phase']==('none' if row['operation']=='enroll' else phase),'Original TC requested direction differs')
    # 独立累计事件，核对停止后的原控制采样器；不把 Applied 次数解释成业务效果次数。
    metrics=data['metrics'];need(set(metrics)==set(OPS),'Control metric dimensions differ')
    for op in OPS:
        group=[r for r in rows if r['operation']==op];wanted=dict(active=0,completed=sum(r['outcome']=='completed' for r in group),unknown_commit=sum(r['outcome']=='unknown_commit' for r in group),before_commit_error=sum(r['outcome']=='before_commit_error' for r in group),cancelled=sum(r['outcome']=='cancelled' for r in group),applied=sum(r['outcome']=='completed' and r['resolution']=='applied' for r in group),unresolved=sum(r['outcome']=='completed' and r['resolution']=='unresolved' for r in group))
        need(set(metrics[op])==set(wanted) and all(type(metrics[op][k]) is int and metrics[op][k]==v for k,v in wanted.items()),'Original events/settled control metrics differ')
    need(any(e['operation']=='enroll' and e['commit_attempted']=='true' for e in rows),'Original enrollment COMMIT missing')
    effect=[e for e in rows if e['operation']==phase and e['resolution']=='applied'];need(effect and effect[0]['commit_attempted']=='true','First original terminal control write not observed')
    frames=[wire.envelope(r) for r in data['frames'] if r.get('event')=='frame'];kind=5 if phase=='rollback' else 3
    callbacks=[r for r in frames if r['kind']==kind and r['direction']=='s2c'];replies=[r for r in frames if r['kind']==kind+1 and r['direction']=='c2s'];need(callbacks and len(callbacks)==len(replies),'Original callback/reply count differs')
    for callback in callbacks:
        identity=wire.callback(callback['body']);need(identity['xid']==observed['xid'] and identity['branch_id']==observed['branch_id'] and identity['resource']==observed['resource_id'],'Original TC callback identity differs')
        reply,=[r for r in replies if (r['connection'],r['request_id'])==(callback['connection'],callback['request_id'])];need(reply['index']>callback['index'] and reply['body'][2:4]==b'\x01\x00' and reply['body'][-1]==(8 if phase=='rollback' else 5),'Original terminal reply differs')
    loss=scenario.endswith('reply_loss');need(sum(r['action']=='drop_and_close' and r['delivery_count']==0 for r in replies)==int(loss),'Original callback reply loss differs');need(all(r['action'] in ['forward','drop_and_close'] for r in replies),'Unexpected reply action')

def controls(data,scenario):
    names=['missing_event','duplicate_event','wrong_xid','wrong_branch','raw_resource','wrong_digest','SQL_payload','wrong_phase','false_unknown','no_commit_marker','metrics_boolean','false_delivery'];rejected=[]
    for name in names:
        v=copy.deepcopy(data);first=v['events'][0]
        if name=='missing_event':v['events'].pop()
        elif name=='duplicate_event':v['events'].append(copy.deepcopy(first))
        elif name=='wrong_xid':first['xid']='other-xid'
        elif name=='wrong_branch':first['branch_id']=str(v['observation']['branch_id']+1)
        elif name=='raw_resource':first['resource_sha256']=v['observation']['resource_id']
        elif name=='wrong_digest':first['resource_sha256']='0'*64
        elif name=='SQL_payload':first['sql']='INSERT INTO private'
        elif name=='wrong_phase':next(r for r in v['events'] if r['operation']!='enroll')['requested_phase']='commit' if scenario.startswith('rollback') else 'rollback'
        elif name=='false_unknown':first['outcome']='unknown_commit'
        elif name=='no_commit_marker':first['commit_attempted']='false'
        elif name=='metrics_boolean':v['metrics']['enroll']['active']=False
        else:
            reply=next(r for r in v['frames'] if r.get('event')=='frame' and r.get('direction')=='c2s' and r.get('kind') in [4,6]);reply['action']='drop_and_close' if reply['action']=='forward' else 'forward';reply['delivery_count']=1-reply['delivery_count']
        try:java(v,scenario)
        except (ValueError,KeyError):rejected.append(name)
        else:raise ValueError('Control-event weakening accepted: '+name)
    return rejected

def sources(root):
    for row in json.loads((root/'source-inputs.json').read_bytes()):need(sha(ROOT/row['path'])==row['sha256'],'Executed source drift: '+row['path'])

def run(n,ns,j,js,m,ms):
    for root,digest in [(n,ns),(j,js),(m,ms)]:verify_catalog(root,digest)
    sources(n);sources(j);result=dict(native=native(n),java=[],independent_external_review='OPEN',WP07_complete=False)
    rows=json.loads((j/'scenarios.json').read_bytes());need(len(rows)==6 and {(r['Rust'],r['scenario']) for r in rows}=={(v,s) for v in ['1.99','1.85'] for s in ['commit','commit_callback_reply_loss','rollback_callback_reply_loss']},'Six original Java profiles required')
    for row in rows:
        folder=ROOT/row['evidence']
        for name,key in [('verified.json','verified_sha256'),('RM-frames.jsonl','frames_sha256'),('mysql-control-events.json','control_events_sha256'),('mysql-control-metrics.json','control_metrics_sha256')]:need(sha(folder/name)==row[key],'Original observation drift')
        data=dict(events=json.loads((folder/'mysql-control-events.json').read_bytes()),metrics=json.loads((folder/'mysql-control-metrics.json').read_bytes()),observation=json.loads((folder/'observation.json').read_bytes()),frames=[json.loads(line) for line in (folder/'RM-frames.jsonl').read_text(encoding='utf-8').splitlines()]);java(data,row['scenario']);result['java'].append(dict(Rust=row['Rust'],scenario=row['scenario'],negative_controls=controls(data,row['scenario'])))
        need(json.loads((folder/'process-cleanup.json').read_bytes())['stopped'] is True,'Original Java process remains')
    mutations=json.loads((m/'mutations.json').read_bytes());need(len(mutations)==6,'Six real event-source mutations required')
    for row in mutations:
        folder=ROOT/row['evidence'];verify_catalog(folder,row['catalog_sha256']);commands=json.loads((folder/'commands.json').read_bytes());need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101,'Mutation did not fail');need(row['marker'].encode() in (folder/(row['Rust']+'-live.stderr')).read_bytes(),'Expected actual event assertion absent');need(json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Mutation runtime remains')
    for root in [n,j]:need(json.loads((root/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Owned runtime remains')
    out=n.parent/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir();save(out/'result.json',dict(status='MYSQL_CONTROL_EVENTS_AND_ORIGINAL_TC_VERIFIED',source_mutations=6,**result));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':
    p=argparse.ArgumentParser()
    for name in ['native','java','mutations']:p.add_argument('--'+name,type=Path,required=True);p.add_argument('--'+name+'-sha',required=True)
    a=p.parse_args();run(a.native,a.native_sha,a.java,a.java_sha,a.mutations,a.mutations_sha)
