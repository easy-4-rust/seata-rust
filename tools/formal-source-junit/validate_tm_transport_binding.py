"""Read-only replay of persisted TC trust, native lock races and original Java wire facts."""
import argparse
import copy
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import struct
import uuid
from inventory_rebuilt import ROOT, need, save, sha
from qualify_rebuilt import verify_catalog
from validate_rm_callback_write_events import wire


def count(value, expected):
    need(type(value) is int and value == expected, 'Counter/version differs or is not an integer')


def binding(value):
    need(type(value) is list and len(value) == 32 and all(type(b) is int and 0 <= b <= 255 for b in value), 'Malformed trust binding')
    return bytes(value)


def peer_binding(roots):
    name=b'seata.example.test'
    anchors=sorted(set(hashlib.sha256(root).digest() for root in roots))
    return hashlib.sha256(b'seata-rust.tc-peer-trust.v1\0TLS1.3.mTLS\0'+struct.pack('>I',len(name))+name+struct.pack('>I',len(anchors))+b''.join(anchors)).digest()


def native(data):
    value=data['downgrade'];count(value['source_schema_version'],2);binding(value['stored_binding']);count(value['status_requests'],0);need(value['rejected'] is True,'Downgrade accepted')
    value=data['claim-race'];need(type(value['observed_mysql_row_waits']) is int and value['observed_mysql_row_waits']>0,'Real MySQL row wait missing')
    count(value['epoch_after_rejection'],1);count(value['final_epoch'],2);count(value['commit_requests'],1);count(value['status_requests'],3)
    need(value['send_started'] is True and value['stopped_owner_verified'] is True,'Stopped-owner/end gate missing')
    need(binding(value['stored_binding'])==binding(value['expected_binding']),'Claim changed trust binding')
    value=data['legacy'];count(value['legacy_schema_version'],1);need(value['legacy_binding'] is None,'Legacy source promoted')
    count(value['legacy_end_requests'],1);count(value['TLS_without_014_source_rows'],0);count(value['TLS_without_014_mysql_error'],1054)
    value=data['restored'];count(value['wrong_binding_status_requests'],0);count(value['status_requests_after_restore'],1);count(value['commit_requests_after_restore'],1)
    need(all(value[k] is True for k in ['missing_column_rejected','null_rejected','short_binding_rejected']),'Protected metadata fallback accepted')
    need(binding(value['restored_binding'])==binding(value['expected_binding']),'Restored bytes differ')
    value=data['grants'];count(value['immutable_binding_updates_denied'],2)
    need(all(value[k] is True for k in ['insert_rejected_before_upgrade','schema_version_update_denied','recovery_insert_denied']),'Role boundaries weakened')
    need(binding(value['binding_after_allowed_epoch_update'])==b'b'*32,'Role rewrote trust')


def native_controls(data):
    labels=['downgrade_status','legacy_downgrade','no_wait','early_claim','duplicate_end','wrong_restored_digest',
            'legacy_promoted','missing_014_allowed','short_digest','boolean_counter','mutable_role','no_stop_proof']
    rejected=[]
    for label in labels:
        value=copy.deepcopy(data)
        if label=='downgrade_status':value['downgrade']['status_requests']=1
        elif label=='legacy_downgrade':value['downgrade']['source_schema_version']=1
        elif label=='no_wait':value['claim-race']['observed_mysql_row_waits']=0
        elif label=='early_claim':value['claim-race']['epoch_after_rejection']=2
        elif label=='duplicate_end':value['claim-race']['commit_requests']=2
        elif label=='wrong_restored_digest':value['restored']['restored_binding'][0]^=1
        elif label=='legacy_promoted':value['legacy']['legacy_schema_version']=2
        elif label=='missing_014_allowed':value['legacy']['TLS_without_014_source_rows']=1
        elif label=='short_digest':value['downgrade']['stored_binding'].pop()
        elif label=='boolean_counter':value['downgrade']['status_requests']=False
        elif label=='mutable_role':value['grants']['immutable_binding_updates_denied']=1
        else:value['claim-race']['stopped_owner_verified']=False
        try:native(value)
        except ValueError:rejected.append(label)
        else:raise ValueError('Native weakening accepted: '+label)
    return rejected


def java(data, expected, wider, client_digest, rollback):
    record=data['binding'];verified=data['verified'];count(record['source_schema_version'],2)
    need(binding(record['stored_binding'])==expected and binding(record['observer_binding'])==expected,'Persisted trust differs from independent DER/SAN hash')
    need(binding(record['wrong_trust_binding'])==wider and wider!=expected,'Widened roots not independently bound')
    need(record['wrong_trust_rejected'] is True and record['original_end_send_marker'] is True,'Wrong trust or send gate accepted')
    count(record['wrong_trust_status_calls'],0);need(record['xid']==verified['xid'],'Original XID drift')
    for key in ['original_tc_end_requests','original_tc_end_success_replies','durable_send_started','original_tc_status_requests']:count(verified[key],1)
    count(verified['delivered_replies'],0);count(verified['durable_end_operation'],2 if rollback else 1)
    need(verified['old_memory_recovery_denied'] is True and verified['tm_call_metrics_verified'] is True,'Original unknown/no-replay result differs')
    parsed=[]
    for row in data['frames']:
        if row.get('event')!='frame':continue
        need(row['action'] in ['forward','drop_original_tm_end_reply'],'Unexpected original TM action')
        # 仅为复用原帧头校验映射已知的零投递动作；原始动作仍逐项核验，不改证据。
        value=wire.envelope(dict(row,action='hold' if row['action']=='drop_original_tm_end_reply' else 'forward'))
        value['action']=row['action'];parsed.append(value)
    pick=lambda kind,direction:[r for r in parsed if r['kind']==kind and r['direction']==direction]
    begin,=pick(1,'c2s');began,=pick(2,'s2c');end,=pick(9 if rollback else 7,'c2s');reply,=pick(10 if rollback else 8,'s2c')
    query,=pick(15,'c2s');status,=pick(16,'s2c')
    key=lambda r:(r['connection'],r['request_id'])
    need(key(begin)==key(began) and key(end)==key(reply) and key(query)==key(status),'Original request/response correlation differs')
    need(begin['connection']==end['connection'] and query['connection']!=end['connection'],'Original owner/observer lifecycle differs')
    need(reply['action']=='drop_original_tm_end_reply' and reply['delivery_count']==0 and reply['body'][2:4]==b'\x01\x00','Original successful end reply not dropped')
    need(query['body'][2:]==end['body'][2:],'Status queried a different XID')
    reader=wire.Reader(end['body']);reader.number('>H');need(reader.string()==record['xid'],'Wire XID differs from source')
    need(status['action']=='forward' and len(status['body'])==5 and status['body'][2:4]==b'\x01\x00','Invalid original status response')
    states={2:'Committing',4:'Rollbacking',8:'AsyncCommitting',9:'Committed',11:'Rollbacked',15:'Finished'}
    need(states.get(status['body'][-1])==verified['observed_status'],'Original status code differs')
    connections=sorted({r['connection'] for r in parsed});need(len(connections)==3,'Original/wrong-trust/observer sessions missing')
    for connection in connections:
        register=[r for r in pick(101,'c2s') if r['connection']==connection]
        registered=[r for r in pick(102,'s2c') if r['connection']==connection]
        need(len(register)==len(registered)==1 and key(register[0])==key(registered[0]) and registered[0]['body'][2]==1,'Each session must complete original TM registration')
    candidates=set(connections)-{begin['connection'],query['connection']};need(len(candidates)==1,'Wrong-trust session ambiguous')
    candidate=candidates.pop();need(all(r['kind'] in [101,102,None] for r in parsed if r['connection']==candidate),'Wrong-trust session sent transaction requests')
    frontend,=data['tls'];need(frontend['role']=='TM' and frontend['mutual_TLS'] is True and frontend['TLS']=='TLSv1.3' and frontend['backend']=='loopback-only','TLS profile differs')
    need(frontend['stopped'] is True and frontend['active_handlers']==0 and frontend['active_writers']==0 and not frontend['handshake_errors'],'TLS workers failed or leaked')
    sessions=frontend['sessions'];need(len(sessions)==3,'Three authenticated TLS sessions required')
    actual=[]
    for session in sessions:
        need(session['TLS']=='TLSv1.3' and session['peer_sha256']==client_digest and session['closed'] is True,'Authenticated session differs')
        need(type(session['upstream_bytes']) is int and type(session['downstream_bytes']) is int,'Invalid TLS byte counter')
        actual.append((session['upstream_bytes'],session['downstream_bytes']))
    wanted=[]
    for connection in connections:
        wanted.append(tuple(sum(len(bytes.fromhex(r['frame_hex'])) for r in parsed if r['connection']==connection and r['direction']==direction and (direction=='c2s' or r['delivery_count']==1)) for direction in ['c2s','s2c']))
    need(sorted(actual)==sorted(wanted),'TLS decrypted bytes differ from delivered original frames')


def java_controls(data,expected,wider,client,rollback):
    labels=['wider_trust_allowed','wrong_binding','client_cert_as_trust','no_mutual_auth','wrong_TLS','wrong_client',
            'forged_bytes','end_reply_delivered','status_replay','wrong_xid','legacy_source','boolean_counter']
    rejected=[]
    for label in labels:
        value=copy.deepcopy(data)
        if label=='wider_trust_allowed':value['binding']['wrong_trust_rejected']=False
        elif label=='wrong_binding':value['binding']['stored_binding']=list(wider)
        elif label=='client_cert_as_trust':value['binding']['stored_binding']=list(bytes.fromhex(client))
        elif label=='no_mutual_auth':value['tls'][0]['mutual_TLS']=False
        elif label=='wrong_TLS':value['tls'][0]['sessions'][0]['TLS']='TLSv1.2'
        elif label=='wrong_client':value['tls'][0]['sessions'][0]['peer_sha256']='0'*64
        elif label=='forged_bytes':value['tls'][0]['sessions'][0]['upstream_bytes']+=1
        elif label=='end_reply_delivered':
            next(r for r in value['frames'] if r.get('action')=='drop_original_tm_end_reply').update(action='forward',delivery_count=1)
        elif label=='status_replay':value['frames'].append(copy.deepcopy(next(r for r in value['frames'] if r.get('kind')==15)))
        elif label=='wrong_xid':value['binding']['xid']='other-xid'
        elif label=='legacy_source':value['binding']['source_schema_version']=1
        else:value['binding']['wrong_trust_status_calls']=False
        try:java(value,expected,wider,client,rollback)
        except (ValueError,KeyError):rejected.append(label)
        else:raise ValueError('Java/TLS weakening accepted: '+label)
    return rejected


def run(native_roots,java_root,java_sha,mutations,mutation_sha):
    verify_catalog(java_root,java_sha);verify_catalog(mutations,mutation_sha)
    result=dict(native={},java=[],mutations=6,independent_external_review='OPEN',WP07_complete=False)
    for version,folder,digest in native_roots:
        verify_catalog(folder,digest)
        data={name:json.loads((folder/'observations'/version/(name+'.json')).read_bytes()) for name in ['downgrade','claim-race','legacy','restored','grants']}
        native(data);result['native'][version]=dict(verified=True,controls=native_controls(data))
        for row in json.loads((folder/'source-inputs.json').read_bytes()):need(sha(ROOT/row['path'])==row['sha256'],'Executed native source drift')
    expected=peer_binding([(java_root/'root.der').read_bytes()]);wider=peer_binding([(java_root/'root.der').read_bytes(),(java_root/'other-root.der').read_bytes()]);client=sha(java_root/'client.der')
    records=json.loads((java_root/'scenarios.json').read_bytes());need(len(records)==4 and len({(r['Rust'],r['scenario']) for r in records})==4,'Four original Java scenarios required')
    for row in records:
        folder=ROOT/row['evidence'];member,=row['stage_files']
        for name,digest in [('verified.json',row['verified_sha256']),('transport-tls.json',row['tls_sha256']),('TM-frames.jsonl',row['tm_frames_sha256']),(member['path'],member['sha256'])]:need(sha(folder/name)==digest,'Original Java primary observation drift')
        data=dict(binding=json.loads((folder/member['path']).read_bytes()),verified=json.loads((folder/'verified.json').read_bytes()),tls=json.loads((folder/'transport-tls.json').read_bytes()),frames=[json.loads(line) for line in (folder/'TM-frames.jsonl').read_text(encoding='utf-8').splitlines()])
        rollback=row['scenario']=='tm_durable_rollback_loss';java(data,expected,wider,client,rollback)
        result['java'].append(dict(Rust=row['Rust'],scenario=row['scenario'],verified=True,controls=java_controls(data,expected,wider,client,rollback)))
        need(json.loads((folder/'process-cleanup.json').read_bytes())['stopped'],'Original TC process remains')
    for row in json.loads((java_root/'source-inputs.json').read_bytes()):need(sha(ROOT/row['path'])==row['sha256'],'Executed Java fixture source drift')
    rows=json.loads((mutations/'mutations.json').read_bytes());need(len(rows)==6,'Missing real source mutation')
    for row in rows:
        folder=ROOT/row['evidence'];verify_catalog(folder,row['catalog_sha256'])
        metadata=json.loads((folder/'result.json').read_bytes());need(metadata['expected_failure'] and metadata['selected_case']==row['selected_case'],'Mutation selected case drift')
        commands=json.loads((folder/'commands.json').read_bytes());need(next(c for c in commands if c['label']==row['Rust']+'-live')['exit_code']==101,'Mutation did not fail in real test')
        raw=(folder/(row['Rust']+'-live.stdout')).read_bytes()+(folder/(row['Rust']+'-live.stderr')).read_bytes();need(metadata['expected_failure'].encode() in raw,'Expected behavioral failure absent')
        need(json.loads((folder/'cleanup.json').read_bytes())['owned_container_removed'],'Mutation runtime leaked')
    need(json.loads((java_root/'cleanup.json').read_bytes())['owned_container_removed'],'Owned Java database remains')
    out=java_root.parent/('validation_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir()
    save(out/'result.json',dict(status='PERSISTED_TC_TRUST_AND_RAW_JAVA_EVIDENCE_VERIFIED',independent_original_binding=list(expected),independent_wider_binding=list(wider),**result))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(validation=str(out),catalog_sha256=sha(out/'catalog.json'))))


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--native-199',type=Path,required=True);p.add_argument('--native-199-sha',required=True);p.add_argument('--native-185',type=Path,required=True);p.add_argument('--native-185-sha',required=True);p.add_argument('--java',type=Path,required=True);p.add_argument('--java-sha',required=True);p.add_argument('--mutations',type=Path,required=True);p.add_argument('--mutations-sha',required=True);a=p.parse_args()
    run([('1.99',a.native_199,a.native_199_sha),('1.85',a.native_185,a.native_185_sha)],a.java,a.java_sha,a.mutations,a.mutations_sha)
