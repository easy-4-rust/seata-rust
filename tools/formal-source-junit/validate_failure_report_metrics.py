"""Independent fixed-counter and original V1 failure-report verification."""
import argparse,copy,datetime,hashlib,json,re,shutil,struct,uuid,zipfile
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha
from qualify_rebuilt import verify_catalog
from validate_business_metrics import OUTCOMES as PRIOR
from validate_rm_callback_write_events import wire
SHAPE=dict(PRIOR)
SHAPE.update(rollback_and_report_failure=['failure_report_skipped','failure_rollback_unconfirmed','failure_report_observed','failure_report_unknown','failure_report_failed','cancelled'],prepare_failure_report=['failure_report_not_eligible','failure_report_already_attempted','failure_report_observed','failure_report_unknown','failure_report_failed','cancelled'],prepare_and_report_failure=['prepared_unregistered','prepared_registered','prepare_failure_unreported','failure_report_observed','failure_report_unknown','failure_report_failed','failure_report_not_eligible','failure_report_already_attempted','cancelled'])
SCENARIOS=['failed','report_loss','undo_error','undo_report_loss','undo_duplicate','enroll_error','enroll_duplicate','enroll_duplicate_report_loss']
def profile(*counts):
    result={op:dict(active=0,outcomes={label:0 for label in labels}) for op,labels in SHAPE.items()}
    for entry in counts:
        path,count=entry if isinstance(entry,tuple) else (entry,1);op,label=path.split('.');result[op]['outcomes'][label]=count
    return result

def exact(actual,expected):
    need(type(actual) is dict and set(actual)==set(SHAPE),'Stage dimensions differ')
    for op,labels in SHAPE.items():
        a=actual[op];need(set(a)=={'active','outcomes'} and type(a['active']) is int and a['active']==0,'Active/fields differ')
        need(set(a['outcomes'])==set(labels),'Outcome dimensions differ')
        for label in labels:need(type(a['outcomes'][label]) is int and a['outcomes'][label]==expected[op]['outcomes'][label],'Counter differs: '+op+'.'+label)

def java_expected(scenario):
    loss=scenario in ['report_loss','undo_report_loss','enroll_duplicate_report_loss'];outcome='failure_report_unknown' if loss else 'failure_report_observed'
    if scenario in ['failed','report_loss']:return profile('execute.execution_completed','prepare.prepared_registered','rollback.rollback_confirmed','rollback_and_report_failure.'+outcome)
    values=['execute.execution_completed','prepare.prepare_failed','rollback.rollback_confirmed','prepare_failure_report.'+outcome,'prepare_and_report_failure.'+outcome]
    if loss:values.append('prepare_failure_report.failure_report_already_attempted')
    return profile(*values)

def java(data,scenario):
    exact(data['counts'],java_expected(scenario));verified=data['verified'];loss=scenario in ['report_loss','undo_report_loss','enroll_duplicate_report_loss']
    for key,value in [('phase_one_done_reports',0),('phase_one_failed_reports',1),('phase_one_report_success_responses',1),('report_response_delivered',int(not loss)),('original_registrations',1),('global_commits',0),('global_rollbacks',1)]:need(type(verified[key]) is int and verified[key]==value,'Original Java decision differs')
    frames=[]
    for row in data['frames']:
        if row.get('event')!='frame':continue
        original=row.get('action');copy_row=dict(row)
        if original=='drop_original_report_reply':copy_row['action']='hold'
        parsed=wire.envelope(copy_row);parsed['action']=original;frames.append(parsed)
    reports=[r for r in frames if r['direction']=='c2s' and r['kind']==13];acks=[r for r in frames if r['direction']=='s2c' and r['kind']==14]
    need(len(reports)==len(acks)==1,'Failure report replay or missing response')
    report,ack=reports[0],acks[0];reader=wire.Reader(report['body']);need(reader.number('>H')==13,'Wrong payload')
    xid=reader.string();branch=reader.number('>q');status=reader.number('>B');resource=reader.string();application_data=reader.string(True);branch_type=reader.number('>B');reader.done()
    need(xid==verified['xid'] and branch==verified['branch_id'] and status==3 and branch_type==0,'Original failed-branch identity/status differs')
    need(resource==data['observation']['resource_id'] and application_data is None,'Original report optional fields differ')
    registrations=[r for r in frames if r['direction']=='c2s' and r['kind']==11];registration_acks=[r for r in frames if r['direction']=='s2c' and r['kind']==12]
    need(len(registrations)==len(registration_acks)==1,'Original registration count differs')
    registration,registration_ack=registrations[0],registration_acks[0]
    original=wire.Reader(registration['body']);need(original.number('>H')==11 and original.string()==xid and original.number('>B')==0 and original.string()==resource and original.string(True)=='undo_work:1' and original.string(True) is None,'Original registration fields differ');original.done()
    issued=wire.Reader(registration_ack['body']);need(issued.number('>H')==12 and issued.take(2)==b'\x01\x00' and issued.number('>q')==branch,'Failure report does not use original TC-issued branch');issued.done()
    need(registration['connection']==registration_ack['connection']==report['connection'] and registration['request_id']==registration_ack['request_id'] and registration['index']<registration_ack['index']<report['index'],'Original registration ownership/order differs')

    need((report['connection'],report['request_id'])==(ack['connection'],ack['request_id']) and report['index']<ack['index'],'Response origin differs')
    need(report['action']=='forward' and report['delivery_count']==1 and ack['body']==b'\x00\x0e\x01\x00','Original request/response bytes differ')
    need(ack['action']==('drop_original_report_reply' if loss else 'forward') and ack['delivery_count']==int(not loss),'Actual original ACK delivery differs')

def controls(data,scenario):
    names=['missing_counter','wrong_stage','active_leak','boolean_counter','missing_outcome','identity_label','false_preparation_success','report_replay','report_delivered','wrong_branch','wrong_status','wrong_response_origin'];rejected=[]
    for label in names:
        v=copy.deepcopy(data);op='rollback_and_report_failure' if scenario in ['failed','report_loss'] else 'prepare_failure_report';outcome='failure_report_unknown' if scenario in ['report_loss','undo_report_loss','enroll_duplicate_report_loss'] else 'failure_report_observed'
        if label=='missing_counter':v['counts'][op]['outcomes'][outcome]=0
        elif label=='wrong_stage':v['counts']['rollback']['outcomes']['rollback_confirmed']=0
        elif label=='active_leak':v['counts'][op]['active']=1
        elif label=='boolean_counter':v['counts'][op]['outcomes']['cancelled']=False
        elif label=='missing_outcome':v['counts'][op]['outcomes'].pop('cancelled')
        elif label=='identity_label':v['counts']['xid']='private'
        elif label=='false_preparation_success':v['counts']['prepare']['outcomes']['prepared_registered']+=1
        else:
            request=next(r for r in v['frames'] if r.get('event')=='frame' and r.get('kind')==13);ack=next(r for r in v['frames'] if r.get('event')=='frame' and r.get('kind')==14)
            if label=='report_replay':v['frames'].append(copy.deepcopy(request))
            elif label=='report_delivered':ack['delivery_count']=1-ack['delivery_count'];ack['action']='forward' if ack['delivery_count'] else 'drop_original_report_reply'
            elif label=='wrong_response_origin':ack['connection']+=1
            else:
                raw=bytes.fromhex(request['frame_hex']);head=int.from_bytes(raw[7:9],'big');body=bytearray(raw[head:]);at=4+int.from_bytes(body[2:4],'big')
                if label=='wrong_branch':
                    v['verified']['branch_id']+=1;body[at:at+8]=struct.pack('>q',v['verified']['branch_id'])
                else:body[at+8]=2
                request['frame_hex']=(raw[:head]+body).hex()
        try:java(v,scenario)
        except (ValueError,KeyError):rejected.append(label)
        else:raise ValueError('Evidence weakening accepted: '+label)
    return rejected

def native(folder):
    base=folder/'observations';result={}
    for version in ['1.99','1.85']:
        root=base/version;common=['execute.execution_completed','prepare.prepared_registered']
        specs=[('case-0','phase-one-stage-*.json',profile(*common,'rollback.rollback_confirmed','rollback_and_report_failure.failure_report_observed')),
          ('case-1','rollback-report-loss.json',profile(*common,'rollback.rollback_confirmed','rollback_and_report_failure.failure_report_unknown')),
          ('case-2','rollback-driver-loss.json',profile(*common,'rollback.rollback_unconfirmed','rollback_and_report_failure.failure_rollback_unconfirmed')),
          ('case-4','automatic-cancelled.json',profile('execute.execution_completed','prepare.prepare_failed','rollback.rollback_confirmed','prepare_and_report_failure.cancelled','prepare_failure_report.cancelled','prepare_failure_report.failure_report_already_attempted')),
          ('case-5','automatic-observed.json',profile('execute.execution_completed','prepare.prepare_failed','rollback.rollback_confirmed','prepare_and_report_failure.failure_report_observed','prepare_failure_report.failure_report_observed')),
          ('case-6','empty-invalid.json',profile('prepare.prepare_failed','prepare.prepared_unregistered','rollback.rollback_confirmed','prepare_and_report_failure.prepare_failure_unreported','prepare_and_report_failure.prepared_unregistered','rollback_and_report_failure.failure_report_skipped')),
          ('case-7','registered-checked.json',profile(*common,'check_commit.checked_without_witness','rollback.rollback_confirmed','prepare_and_report_failure.prepared_registered','rollback_and_report_failure.failure_report_observed'))]
        for mode,outcome in [('observed','failure_report_observed'),('rejected','failure_report_failed'),('lost','failure_report_unknown'),('cancelled','cancelled')]:specs.append(('case-3','explicit-'+mode+'.json',profile('execute.execution_completed','prepare.prepare_failed','rollback.rollback_confirmed','prepare_failure_report.'+outcome,'prepare_failure_report.failure_report_already_attempted')))
        for case,name,expected in specs:
            files=list((root/case).glob(name));need(len(files)==1,'Selected native observation missing');exact(json.loads(files[0].read_bytes()),expected)
        # 原 HTTP 来源不启用实验报告操作；新增阶段应准确导出为零。
        for suffix in ['zero','after-close']:
            text=(root/'http'/('phase-one-http-'+suffix+'.txt')).read_text(encoding='utf-8');lines=text.splitlines();need(len(lines)==63 and len(set(lines))==63,'HTTP dimensions/repetition differ')
            expected=profile() if suffix=='zero' else profile('prepare.prepare_failed','execute.execution_failed','rollback.rollback_confirmed')
            wanted={}
            for op,labels in SHAPE.items():
                wanted['seata_mysql_phase_one_active{operation="'+op+'"}']=0
                for outcome in labels:wanted['seata_mysql_phase_one_total{operation="'+op+'",outcome="'+outcome+'"}']=expected[op]['outcomes'][outcome]
            actual={k:int(v) for line in lines for k,v in [line.rsplit(' ',1)]};need(actual==wanted,'HTTP values/labels differ')
        result[version]=dict(native_profiles=len(specs),HTTP_series=63)
    return result

def sources(folder,allow_test_lint=False):
    drift=[]
    for row in json.loads((folder/'source-inputs.json').read_bytes()):
        current=ROOT/row['path']
        if sha(current)==row['sha256']:continue
        need(allow_test_lint and row['path']=='crates/seata_db_mysql/tests/support/first_phase_fixture.rs','Executed production/test source drift: '+row['path'])
        with zipfile.ZipFile(folder/'source-inputs.zip') as archive:old=archive.read(row['path'])
        need(hashlib.sha256(old).hexdigest()==row['sha256'],'Archived helper source differs')
        needle=b'    pub fn save_phase_metrics(&self, name: &str) {';ending=b'\r\n' if old.count(b'\r\n')==old.count(b'\n') else b'\n'
        need(old.count(needle)==1 and current.read_bytes()==old.replace(needle,b'    #[allow(dead_code)]'+ending+needle),'Shared test helper drift is not the exact diagnostic-only attribute')
        drift.append(dict(path=row['path'],archived_sha256=row['sha256'],current_sha256=sha(current),change='one allow(dead_code) attribute on shared native test snapshot helper; no runtime body change'))
    return drift

def run(native_root,native_sha,java_root,java_sha,mutations,mutation_sha):
    verify_catalog(native_root,native_sha);verify_catalog(java_root,java_sha);verify_catalog(mutations,mutation_sha)
    result=dict(native=native(native_root),java=[],independent_external_review='OPEN',WP07_complete=False)
    sources(native_root);result['Java_shared_test_helper_lint_only']=sources(java_root,True)
    records=json.loads((java_root/'scenarios.json').read_bytes());need(len(records)==16 and {(r['Rust'],r['scenario']) for r in records}=={(v,s) for v in ['1.99','1.85'] for s in SCENARIOS},'Original Java case set differs')
    for row in records:
        root=ROOT/row['evidence'];need(sha(root/'verified.json')==row['verified_sha256'] and sha(root/'RM-frames.jsonl')==row['frames_sha256'],'Original Java primary observation drift')
        member,=row['stage_files'];need(sha(root/member['path'])==member['sha256'],'Actual counter snapshot drift')
        data=dict(counts=json.loads((root/member['path']).read_bytes()),verified=json.loads((root/'verified.json').read_bytes()),observation=json.loads((root/'observation.json').read_bytes()),frames=[json.loads(line) for line in (root/'RM-frames.jsonl').read_text(encoding='utf-8').splitlines()]);java(data,row['scenario']);result['java'].append(dict(Rust=row['Rust'],scenario=row['scenario'],controls=controls(data,row['scenario'])))
        need(json.loads((root/'process-cleanup.json').read_bytes())['stopped'] is True,'Original Java process remains')
    rows=json.loads((mutations/'mutations.json').read_bytes());need(len(rows)==6,'Six real source mutations required')
    for row in rows:
        root=ROOT/row['evidence'];verify_catalog(root,row['catalog_sha256']);commands=json.loads((root/'commands.json').read_bytes());need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101,'Mutation did not fail');need(row['marker'].encode() in (root/(row['Rust']+'-live.stderr')).read_bytes(),'Behavioral assertion missing')
        stderr=(root/(row['Rust']+'-live.stderr')).read_bytes().replace(b'\\',b'/');span=re.search(rb'support/first_phase_fixture\.rs:(\d+):\d+:',stderr);need(span is not None and b'right: 1' in stderr,'Mutation did not fail at the original counter assertion')
        with zipfile.ZipFile(root/'source-inputs.zip') as archive:helper=archive.read('crates/seata_db_mysql/tests/support/first_phase_fixture.rs').splitlines()
        at=int(span.group(1))-1;context=b'\n'.join(helper[at:at+14]);need(helper[at].strip()==b'assert_eq!(' and b'.snapshot(operation)' in context and b'.count(outcome)' in context and b'expected' in context,'Archived failed span is not the metric assertion')
        need(json.loads((root/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Mutation database remains')
    for root in [native_root,java_root]:need(json.loads((root/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Owned database remains')
    out=native_root.parent/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir();save(out/'result.json',dict(status='FAILURE_REPORT_COUNTERS_AND_RAW_ORIGINAL_TC_VERIFIED',source_mutations=len(rows),**result));shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':
    p=argparse.ArgumentParser()
    for name in ['native','java','mutations']:p.add_argument('--'+name,type=Path,required=True);p.add_argument('--'+name+'-sha',required=True)
    a=p.parse_args();run(a.native,a.native_sha,a.java,a.java_sha,a.mutations,a.mutations_sha)
