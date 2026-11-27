"""Independently bind original Undo events to raw MySQL packets and durable state."""
import argparse
import copy
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import uuid
import zipfile
from inventory_rebuilt import ROOT, need, save, sha, safe_path
from mysql_undo_executor_live import IMAGE


def catalog(folder, digest):
    need(sha(folder/'catalog.json') == digest, 'Trusted catalog digest differs')
    rows = json.loads((folder/'catalog.json').read_bytes())
    need(len(rows) == len({r['path'] for r in rows}), 'Duplicate catalog member')
    need({r['path'] for r in rows} == {p.relative_to(folder).as_posix() for p in folder.rglob('*') if p.is_file() and p.name != 'catalog.json'}, 'Uncatalogued evidence')
    for row in rows:
        need(sha(safe_path(folder, row['path'])) == row['sha256'], 'Catalog member changed')


def events(values, resource, expected):
    need(len(values) == len(expected), 'Missing/duplicate event')
    fields = {'target','message','operation','xid','branch_id','resource_sha256','outcome','observation','commit_attempted'}
    for event, boundary in zip(values, expected, strict=True):
        need(set(event) == fields and all(type(v) is str for v in event.values()), 'Unexpected event field or type')
        need(event['target'] == 'seata_db_mysql::undo' and event['message'] == 'mysql_undo_outcome', 'Wrong event target')
        need(event['xid'] == 'rollback:xid' and event['branch_id'] == '42', 'Original identity changed')
        need(event['resource_sha256'] == hashlib.sha256(resource.encode()).hexdigest(), 'Wrong resource correlation')
        need(tuple(event[k] for k in ['operation','outcome','observation','commit_attempted']) == boundary, 'Incorrect outcome/COMMIT boundary')
        text=json.dumps(event)
        need(not any(secret in text for secret in ['jdbc:mysql','undo-event-secret','seata-mysql-fixture-only','INSERT INTO','private-password']), 'Sensitive material logged')


def packet(frame, sequence):
    need(type(frame) is list and all(type(b) is int and 0 <= b <= 255 for b in frame), 'Raw packet bytes missing')
    frame=bytes(frame)
    need(len(frame)>=5 and int.from_bytes(frame[:3],'little') == len(frame)-4 and frame[3] == sequence, 'Invalid physical MySQL packet')
    return frame[4:]


def validate_native(folder):
    source_rows=json.loads((folder/'source-inputs.json').read_bytes())
    with zipfile.ZipFile(folder/'source-inputs.zip') as archive:
        need(set(archive.namelist()) == {r['path'] for r in source_rows}, 'Source archive mismatch')
        for row in source_rows:
            need(hashlib.sha256(archive.read(row['path'])).hexdigest() == row['sha256'] == sha(safe_path(ROOT,row['path'])), 'Executed source differs from current source')
    commands=json.loads((folder/'commands.json').read_bytes())
    need(all(c['exit_code']==0 for c in commands if c['label']!='confirm-removed'), 'Native command failed')
    need(next(c for c in commands if c['label']=='confirm-removed')['exit_code'] != 0, 'Container still present')
    state=json.loads((folder/'inspect.stdout').read_bytes())[0]
    need(state['Id'] == (folder/'create.stdout').read_text().strip() and state['Image'] == IMAGE, 'Owned MySQL identity mismatch')
    need(len(state['Config']['Labels']['seata.undo.events.owner']) == 32, 'Missing fixture owner')
    need(state['HostConfig']['PortBindings']['3306/tcp'][0]['HostIp']=='127.0.0.1','Nonloopback database fixture')
    need(json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Owned cleanup missing')
    rejected=[]
    for version in ['1.99','1.85']:
        for index in range(3):
            output=(folder/(version+'-case-'+str(index)+'.stdout')).read_bytes()
            need(b'test result: ok. 1 passed;' in output, 'Actual target summary missing')
        contexts={}
        for p in (folder/'observations'/version).rglob('undo-context-*.json'):
            context=json.loads(p.read_bytes());need(context['xid']=='rollback:xid' and context['branch_id']==42,'Context identity mismatch')
            contexts[hashlib.sha256(context['resource_id'].encode()).hexdigest()]=context['resource_id']
        def check(value, expected):
            need(value and value[0]['resource_sha256'] in contexts, 'Missing independent resource context')
            resource=contexts[value[0]['resource_sha256']]
            events(value,resource,expected)
            return resource
        def read(case,name):return json.loads((folder/'observations'/version/('case-'+str(case))/name).read_bytes())
        value=read(0,'undo-events-observations.json')
        expected=[('fence','completed','undo_present','false'),('rollback','completed','consumed','true'),('fence','completed','fenced_inserted','true'),('fence','completed','fenced_existing','false'),('rollback','completed','fenced_existing','false')]
        resource=check(value['events'],expected);need((value['business'],value['undo'])==(0,1),'Compensation/fence final state mismatch')
        controls=[]
        base=value['events']
        controls.extend([('missing',base[:-1]),('duplicate',base+[base[0]])])
        for key,replacement in [('xid','other:xid'),('branch_id','43'),('resource_sha256','0'*64),('resource_sha256',resource),('sql','INSERT INTO private'),('commit_attempted','true'),('outcome','unknown_commit'),('observation','consumed')]:
            changed=copy.deepcopy(base);changed[0][key]=replacement;controls.append((key+'-'+replacement,changed))
        changed=copy.deepcopy(base);changed[0]['commit_attempted']=False;controls.append(('boolean-field',changed))
        for label,changed in controls:
            try:events(changed,resource,expected)
            except (AssertionError,RuntimeError,ValueError):rejected.append(version+':'+label)
            else:raise AssertionError('Weakened evidence accepted: '+label)
        for operation in ['rollback','fence']:
            value=read(1,'undo-events-'+operation+'-wait.json')
            check(value['events'],[(operation,'cancelled','not_observed','false'),(operation,'before_commit_error','not_observed','false')])
            need((value['business'],value['undo'])==(1,1),'Cancelled wait changed durable data')
            for outcome in ['unknown','cancelled']:
                value=read(2,'undo-events-'+operation+'-'+outcome+'.json')
                check(value['events'],[(operation,'unknown_commit' if outcome=='unknown' else 'cancelled','not_observed','true'),(operation,'completed','fenced_inserted' if operation=='rollback' else 'fenced_existing','true' if operation=='rollback' else 'false')])
                need(packet(value['actual_commit_packet'],0)==b'\x03COMMIT','Actual COMMIT missing')
                ok=packet(value['actual_ok_packet'],1)
                need(len(ok)>=7 and ok[:3]==b'\x00\x00\x00' and int.from_bytes(ok[3:5],'little')&3==2,'Original COMMIT OK not observed')
                need(value['before_recovery']==([0,0] if operation=='rollback' else [1,1]),'Server did not commit before unknown/cancellation')
                need(value['after_recovery']==([0,1] if operation=='rollback' else [1,1]),'Same identity recovery state mismatch')
    return rejected


def validate_mutations(folder):
    rows=json.loads((folder/'mutations.json').read_bytes())
    need({(r['Rust'],r['operation']) for r in rows} == {(v,o) for v in ['1.99','1.85'] for o in ['finish_event','rollback_commit_marker','fence_commit_marker','cancel_event']} and len(rows)==8,'Real source mutations incomplete')
    for row in rows:
        child=safe_path(ROOT,row['evidence']);catalog(child,row['catalog_sha256'])
        commands=json.loads((child/'commands.json').read_bytes())
        need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101,'Mutation did not fail target')
        text=(child/(row['Rust']+'-live.stdout')).read_bytes()+(child/(row['Rust']+'-live.stderr')).read_bytes()
        need(row['marker'].encode() in text,'Mutation failed for an unrelated reason')
        need(json.loads((child/'cleanup.json').read_bytes())['owned_container_removed'] is True,'Mutation cleanup missing')
    restored=json.loads((folder/'restored.json').read_bytes())
    need(all(r['exact_original_restored'] is True and sha(safe_path(ROOT,r['path'])) == r['sha256'] for r in restored),'Original source not restored')


def run(args):
    native=ROOT/args.native;mutations=ROOT/args.mutations
    catalog(native,args.native_sha);catalog(mutations,args.mutations_sha)
    rejected=validate_native(native);validate_mutations(mutations)
    out=ROOT/'docs/phase-2/evidence/mysql-undo-events'/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    save(out/'result.json',dict(status='INDEPENDENT_UNDO_EVENTS_VALIDATED',native=args.native,native_catalog_sha256=args.native_sha,mutations=args.mutations,mutations_catalog_sha256=args.mutations_sha,rejected_evidence_controls=rejected,source_mutations=8,current_source_bytes_match=True,external_review='OPEN',WP07_complete=False))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for key in ['native','native-sha','mutations','mutations-sha']:parser.add_argument('--'+key,required=True)
    run(parser.parse_args())
