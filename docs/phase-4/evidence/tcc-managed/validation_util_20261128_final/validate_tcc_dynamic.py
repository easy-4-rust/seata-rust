"""Verify raw Registered reports, immutable versions, original callback identity and real business effects."""
import argparse
import copy
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import struct

from inventory_rebuilt import need
from validate_structured_logs import catalog, sources
from validate_xa_managed import string, only
from validate_xa_java_faults import frames
from xa_managed_java_faults import guard

MODES = ['delayed','immediate','precision']
NAMES = ['dynamic-reports','dynamic-report-errors','dynamic-report-integrity','dynamic-report-setup']
INSERT = b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(1,7)'
DELETE = b'Execute\tDELETE FROM tcc_managed WHERE id=1'


def load(path):return json.loads(path.read_bytes())
def decoded(app):return json.loads(app,parse_float=Decimal)


def expected_app(app,mode,final):
    expected = dict(useTCCFence=True,id=1,amount=7 if mode=='precision' else 9,precise=Decimal('1.2300000000000000000000000000'+('2' if mode=='precision' else '1')))
    if mode!='precision':expected.update(object='{"a":1,"precise":1.23000000000000000000000000001}',array='[1,2]')
    if mode=='immediate' and final:expected['final']=9
    need(decoded(app)==dict(actionContext=expected),'Actual report dropped/rounded/replaced dynamic values')
    need(('"precise":1.2300000000000000000000000000'+('2' if mode=='precision' else '1')) in app,'Original decimal bytes lost')


def context_versions(raw,xid,branch,original,reports):
    rows=[line.split(b'\t') for line in raw.splitlines()]
    need(len(rows)==len(reports)+1,'Report versions missing or replayed')
    expected_uuid = None
    for revision,row in enumerate(rows):
        need(len(row)==9,'Context observation shape differs')
        hxid,bid,rev,resource,registration,application,server,schema,digest=row
        app=original if revision==0 else reports[revision-1]
        need(hxid.decode()==xid.encode().hex().upper() and int(bid)==branch and int(rev)==revision and bytes.fromhex(resource.decode())==b'tcc-original-action' and bytes.fromhex(registration.decode())==original.encode() and bytes.fromhex(application.decode())==app.encode() and schema==b'1','Original context identity/version/payload changed')
        uuid=bytes.fromhex(server.decode());need(len(uuid)==36,'Original server UUID missing')
        if expected_uuid is None:expected_uuid=uuid
        need(uuid==expected_uuid,'Report versions changed original MySQL instance')
        value=hashlib.sha256(b'seata-rs-tcc-context-v1')
        for part in [xid.encode(),b'tcc-original-action',original.encode(),app.encode(),uuid]:value.update(struct.pack('>Q',len(part)));value.update(part)
        value.update(struct.pack('>qH',branch,revision))
        need(digest.decode()==value.hexdigest().upper(),'Original context SHA-256 does not bind actual bytes/identity')


def mysql_case(folder,version,index,alterations=None):
    alterations=alterations or {};prefix=version+'-case-'+str(index)
    observations=folder/'observations'/version/('case-'+str(index))
    proof=alterations.get('proof',load(observations/'proof'/(NAMES[index]+'.json')))
    def fact(name):return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    need(b'test result: ok. 1 passed;' in (folder/(prefix+'.stdout')).read_bytes(),'Dynamic native test did not execute')
    need(type(proof['rust_process_id']) is int and proof['rust_process_id']>0 and fact('undo')==b'0','Native identity missing or Java Undo changed')
    if index==0:
        rows=proof['cases'];need(len(rows)==6,'Dynamic normal matrix incomplete')
        dml=[]
        for i,row in enumerate(rows):
            mode=MODES[i//2];commit=i%2==0
            need(row['immediate'] is (mode=='immediate') and row['precision_only'] is (mode=='precision') and row['phase']==('Commit' if commit else 'Rollback') and row['old_pending'] is commit and row['journal']=='Completed' and row['versions']==(3 if mode=='immediate' else 2),'Original Pending/report versions not recovered')
            need(len(row['reports'])==(2 if mode=='immediate' else 1),'Original report sent wrong number of times')
            for j,app in enumerate(row['reports']):expected_app(app,mode,j==len(row['reports'])-1)
            dml.extend([INSERT,b'Execute\tUPDATE tcc_managed SET v='+ (b'14' if mode=='precision' else b'18')+b' WHERE id=1' if commit else DELETE])
        last=rows[-1];xid,branch,status=last['xid'],9501,3;original,reports=last['original_app'],last['reports'];business=b''
    elif index==1:
        rows=proof['cases'];modes=['rejected','lost','caught','cancelled','business_false','business_error']
        need([r['mode'] for r in rows]==modes,'Report failure/cancellation matrix incomplete')
        for i,row in enumerate(rows):
            need(row['original_receipt_retained'] and row['report_unknown'] is (i==1) and row['report_acknowledged_before_try_result'] is (i>=4) and row['reports']==1 and row['try_business_rows']==0 and row['fence_status']==4 and row['versions']==2 and row['journal']=='Completed','Failure/cancellation published business or lost typed report boundary')
            expected_app(row['report_app'],'delayed',True)
        last=rows[-1];xid,branch,status=last['xid'],9605,4;business=b'';dml=[INSERT]*6
        original='{"actionContext":{"useTCCFence":true,"id":1,"amount":7}}';reports=[last['report_app']]
    elif index==2:
        need(proof['rejected']==['payload','registration','resource','instance','version','checksum','high_revision','gap','missing_manifest','extra_index','changed_default','nullable','engine','trigger'] and proof['old_known_context_retryable'] and proof['completed_reuse_rejected_bad_context'] and proof['original_bytes_restored_same_branch_completed'] and proof['static_downgrade_rejected'] and proof['static_completed_downgrade_rejected'] and proof['inconsistent_terminal_fence_not_reused'] and proof['fence_protocol']==2 and proof['journal']=='Completed' and proof['business']==[[1,18]],'Modified report reused completion or original restoration did not finish')
        xid,branch,status=proof['xid'],9701,2;business=b'1\t18';dml=[INSERT,b'Execute\tUPDATE tcc_managed SET v=18 WHERE id=1']
        original='{"actionContext":{"useTCCFence":true,"id":1,"amount":7}}';reports=proof['reports'];need(len(reports)==2 and proof['versions']==3,'Integrity context history changed')
    else:
        rows=proof['cases'];need([r['mode'] for r in rows]==['missing_migration','small_pool','disabled'],'Setup rejection matrix incomplete')
        for i,row in enumerate(rows):
            need(row['original_register_requests']==(0 if i==1 else 1) and row['reports']==row['try_business_rows']==row['versions']==0 and row['journal']==('None' if i==1 else 'Completed') and row['migration_installed_explicitly_for_recovery'] is (i==0),'Setup rejection registered/sent/replayed business or auto-migrated')
        xid,branch,status=rows[-1]['xid'],9802,4;business=b'';dml=[]
        need(not fact('context'),'Setup created dynamic metadata')
    need(fact('fence')==(xid.encode().hex().upper()+'\t'+str(branch)+'\t'+str(status)).encode() and fact('business')==business and fact('business-dml').splitlines()==dml,'Independent Fence/business/DML contradict dynamic lifecycle')
    need(fact('fence-protocol')==(b'1' if index==3 else b'2'),'Fence protocol downgrade was accepted')
    if index!=3:context_versions(fact('context'),xid,branch,original,reports)


def java_case(folder,version,mode,direction,alterations=None):
    alterations=alterations or {};prefix=version+'-'+mode+'-'+direction;case=folder/prefix;commit=direction=='commit'
    proof=alterations.get('proof',load(case/'observations/proof/original-java-tcc-dynamic.json'))
    need(proof['mode']==mode and proof['branch_type']==1 and proof['resource_id']=='tcc-original-action' and proof['global_status']==('Committed' if commit else 'Rollbacked') and proof['status']==(2 if commit else 3) and proof['journal']=='Completed' and proof['finished_inferred_as_success'] is False,'Dynamic TC end inferred instead of observed')
    need(load(case/'client-command.json')['exit_code']==0 and load(case/'process-cleanup.json')['stopped'],'Dynamic Java client or cleanup failed')
    rm,summary,_=frames(case/'RM-frames.jsonl');tm,tm_summary,_=frames(case/'TM-frames.jsonl');rm=alterations.get('rm',rm)
    need(summary['mode']==tm_summary['mode']=='basic' and all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'Dynamic report/decision was rewritten')
    wire=[(r['kind'],r['body'],r['direction']) for r in rm]
    register=only(wire,11,'c2s');xid,pos=string(register,2);need(register[pos]==1,'Registration is not TCC');resource,pos=string(register,pos+1);lock,pos=string(register,pos,4);original,pos=string(register,pos,4)
    need(xid==proof['xid'] and resource=='tcc-original-action' and lock is None and original==proof['registered_application_data'] and pos==len(register),'Original signed registration was rewritten')
    registered=only(wire,12,'s2c');branch=int.from_bytes(registered[4:12],'big',signed=True);need(registered[2:4]==b'\x01\x00' and str(branch)==proof['branch_id'],'Original positive branch receipt differs')
    reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];replies=[r for r in rm if r['kind']==14 and r['direction']=='s2c']
    need(len(reports)==len(replies)==(2 if mode=='immediate' else 1),'Report was replayed or skipped')
    apps=[]
    for index,row in enumerate(reports):
        body=row['body'];value,pos=string(body,2);bid=int.from_bytes(body[pos:pos+8],'big',signed=True);pos+=8
        need(value==xid and bid==branch and body[pos]==1,'Report changed branch or Registered status')
        value,pos=string(body,pos+1);app,pos=string(body,pos,4)
        need(value is None and body[pos:]==b'\x01','Java optional resource or TCC report mode changed')
        expected_app(app,mode,index==len(reports)-1);apps.append(app)
        reply=[r for r in replies if (r['connection'],r['request_id'])==(row['connection'],row['request_id'])]
        need(len(reply)==1 and reply[0]['body']==b'\x00\x0e\x01\x00','Report response not original successful correlation')
    need(apps[-1]==proof['reported_application_data'],'Reported callback payload differs from durable version')
    callbacks=[r for r in rm if r['kind']==(3 if commit else 5)];responses=[r for r in rm if r['kind']==(4 if commit else 6)]
    need(callbacks and len(callbacks)==len(responses),'Actual original TC callback not acknowledged')
    for row in callbacks:
        body=row['body'];value,pos=string(body,2);need(value==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==1,'Dynamic callback changed original branch')
        value,pos=string(body,pos+9);data,pos=string(body,pos,4);need(value==resource and data==apps[-1] and pos==len(body),'TC callback did not carry latest original report bytes')
        reply=[r for r in responses if (r['connection'],r['request_id'])==(row['connection'],row['request_id'])];need(len(reply)==1,'Callback response association differs')
        body=reply[0]['body'];value,pos=string(body,4);need(body[2:4]==b'\x01\x00' and value==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8:]==bytes([5 if commit else 8]),'Dynamic callback terminal response differs')
    tm_wire=[(r['kind'],r['body'],r['direction']) for r in tm];only(tm_wire,1,'c2s');begin=only(tm_wire,2,'s2c');need(string(begin,4)[0]==xid and begin[2:4]==b'\x01\x00','Original global Begin differs')
    need(string(only(tm_wire,7 if commit else 9,'c2s'),2)[0]==xid and not any(r['kind'] in [15,16] for r in tm),'Global End repeated or status query inferred outcome')
    end=only(tm_wire,8 if commit else 10,'s2c');need(end[2:4]==b'\x01\x00' and end[-1]==(9 if commit else 11),'Global success was not explicit')
    def fact(name):return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    need(fact('fence-protocol')==b'2','Original Java dynamic branch lost protocol marker')
    context_versions(fact('independent-report-context'),xid,branch,original,apps)
    need(fact('independent-report-tc')==(xid.encode().hex().upper()+'\t'+str(branch)+'\tTCC\t1\t'+apps[-1].encode().hex().upper()).encode(),'Independent original TC database did not persist new application data')
    need(fact('independent-report-business')==b'1\t7','Business not TRIED at observed report boundary')
    value=14 if mode=='precision' else 18
    need(proof['business']==([[1,value]] if commit else []) and fact('independent-business')==(('1\t'+str(value)).encode() if commit else b'') and fact('independent-state')==(xid.encode().hex().upper()+'\t'+str(branch)+'\t'+('2' if commit else '3')).encode() and fact('independent-tc')==b'0\t0\t0' and not fact('independent-prepared') and fact('native-commands')==b'Query\tXA RECOVER CONVERT XID','Independent phase-two/Fence/TC cleanup differs')
    need(fact('business-dml').splitlines()==[INSERT,b'Execute\tUPDATE tcc_managed SET v='+str(value).encode()+b' WHERE id=1' if commit else DELETE],'Dynamic business DML replayed or used old values')


def verify(mysql,java):
    for folder,digest in [mysql,java]:
        catalog(folder,digest);sources(folder);result=load(folder/'result.json');need(result['Rust_versions']==['1.99','1.85'] and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False and load(folder/'cleanup.json')['owned_container_removed'],'Dynamic scope or cleanup differs')
    result=load(java[0]/'result.json');need(result['modes']==MODES and result['Java_pin']==guard.reg.PIN and result['Java_jar_sha256']==guard.reg.JAR_SHA,'Original Java dynamic matrix/pin differs')
    for version in ['1.99','1.85']:
        for index in range(4):mysql_case(mysql[0],version,index)
        for mode in MODES:
            for direction in ['commit','rollback']:java_case(java[0],version,mode,direction)


def verify_mutations(folder,digest):
    catalog(folder,digest);sources(folder)
    result=load(folder/'result.json');need(result['variants_per_version']==6 and result['actual_behavioral_failures']==12 and result['live_source_mutated'] is result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False,'Dynamic source counterexample scope changed')
    expected={'checksum_ignored','report_error_ignored','AT_status_in_TCC_report','stable_identity_key_bypass','protocol_downgrade_accepted','cached_completion_unverified'}
    records=load(folder/'records.json');need(len(records)==12 and {(r['Rust'],r['mutation']) for r in records}=={(version,name) for version in ['1.99','1.85'] for name in expected},'Dynamic source counterexamples missing')
    cleanup=load(folder/'cleanup.json');need(cleanup['owned_container_removed'] and Path(cleanup['isolated_source']).drive.upper()=='D:','Counterexamples touched live workspace or left database')
    targets=[Path(p).resolve() for p in cleanup['isolated_targets']]
    need(len(targets)==14 and len(set(targets))==14,'Counterexamples did not use independent build directories')
    commands=load(folder/'commands.json')
    artifacts=load(folder/'artifacts.json')
    for version in ['1.99','1.85']:
        for index in range(4):need(b'test result: ok. 1 passed;' in (folder/(version+'-baseline-'+str(index)+'.stdout')).read_bytes(),'Unmodified isolated baseline failed')
    for row in records:
        prefix=row['Rust']+'-'+row['mutation'];need(row['exit_code']==101 and sha_local(folder/(prefix+'.rs'))==row['modified_source_sha256'],'Counterexample source differs')
        raw=(folder/(prefix+'.stdout')).read_bytes();need(b'test result: FAILED. 0 passed; 1 failed;' in raw and row['test'].encode() in raw,'Compiler/setup failure counted as behavior')
        signatures={
            'checksum_ignored':[b'assertion failed: !matches!(adapter.recover',b'Ok(BranchResolution::Applied)'],
            'report_error_ignored':[b'called `Result::unwrap_err()` on an `Ok` value: RegisteredBranch'],
            'AT_status_in_TCC_report':[b'left: PhaseOneDone',b'right: Registered'],
            'protocol_downgrade_accepted':[b'static mode must reject report-enabled records before using old parameters'],
            'cached_completion_unverified':[b'left: (Success, 5)',b'right: (Failed, 6)'],
            'stable_identity_key_bypass':[b'assertion failed: matches!(result, Err(RmError::IdentityConflict))'],
        }
        stderr=(folder/(prefix+'.stderr')).read_bytes();need(all(signature in stderr for signature in signatures[row['mutation']]),'Selected behavior assertion missing; arbitrary setup panic is not a counterexample')
        command=[c for c in commands if c['label']==prefix];need(len(command)==1 and command[0]['exit_code']==101 and row['test'] in command[0]['argv'],'Counterexample did not execute selected behavior')
        artifact=Path(command[0]['argv'][0]).resolve();need(any(artifact.is_relative_to(target) for target in targets),'Mutated artifact escaped isolated D target')
        copies=[a for a in artifacts if a['label']==prefix+'-build'];need(len(copies)==1 and Path(copies[0]['executed_path']).resolve()==artifact and copies[0]['sha256']==row['artifact_sha256'] and any(target.name=='variant-target-'+prefix and Path(copies[0]['built_path']).resolve().is_relative_to(target) and artifact.is_relative_to(target) for target in targets),'Executed isolated copy not bound to actual unique build artifact')


def sha_local(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def weakenings(mysql,java):
    rejected=0
    for version in ['1.99','1.85']:
        for index in range(4):
            proof=load(mysql/'observations'/version/('case-'+str(index))/'proof'/(NAMES[index]+'.json'))
            variants=[dict(facts={name:value}) for name,value in [('business',b'999\t999'),('fence',b''),('undo',b'1'),('business-dml',b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(999,999)'),('context',b'changed'),('fence-protocol',b'0')]]
            changed=copy.deepcopy(proof)
            if index==0:changed['cases'][0]['old_pending']=False
            elif index==1:changed['cases'][0]['original_receipt_retained']=False
            elif index==2:changed['completed_reuse_rejected_bad_context']=False
            else:changed['cases'][1]['original_register_requests']=1
            variants.append(dict(proof=changed))
            if index==2:
                for field in ['static_downgrade_rejected','static_completed_downgrade_rejected','inconsistent_terminal_fence_not_reused']:
                    changed=copy.deepcopy(proof);changed[field]=False;variants.append(dict(proof=changed))
            for alteration in variants:
                try:mysql_case(mysql,version,index,alteration)
                except ValueError:rejected+=1
                else:raise RuntimeError('Dynamic MySQL evidence weakening accepted')
        for mode in MODES:
            for direction in ['commit','rollback']:
                prefix=version+'-'+mode+'-'+direction;proof=load(java/prefix/'observations/proof/original-java-tcc-dynamic.json')
                variants=[dict(facts={name:value}) for name,value in [('independent-report-tc',b'old'),('independent-report-context',b''),('independent-business',b'999\t999'),('business-dml',INSERT),('independent-tc',b'1\t0\t0'),('fence-protocol',b'1')]]
                for key,value in [('global_status','Finished'),('reported_application_data','{}'),('branch_type',2)]:changed=copy.deepcopy(proof);changed[key]=value;variants.append(dict(proof=changed))
                rm,_,_=frames(java/prefix/'RM-frames.jsonl')
                for field in ['status','type','context']:
                    changed=copy.deepcopy(rm);row=next(r for r in changed if r['kind']==13);body=bytearray(row['body']);_,pos=string(bytes(body),2)
                    if field=='status':body[pos+8]=2
                    elif field=='type':body[-1]=2
                    else:body=body.replace(b'"id":1',b'"id":2')
                    row['body']=bytes(body);variants.append(dict(rm=changed))
                for alteration in variants:
                    try:java_case(java,version,mode,direction,alteration)
                    except ValueError:rejected+=1
                    else:raise RuntimeError('Dynamic Java evidence weakening accepted')
    need(rejected==206,'Dynamic evidence weakening matrix incomplete');return rejected


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for name in ['mysql','java']:parser.add_argument('--'+name,type=Path,required=True);parser.add_argument('--'+name+'-sha',required=True)
    parser.add_argument('--weakenings',action='store_true');args=parser.parse_args();verify((args.mysql,args.mysql_sha),(args.java,args.java_sha))
    if args.weakenings:print('Rejected dynamic report evidence weakenings:',weakenings(args.mysql,args.java))
    print('Verified eight actual MySQL test functions / 32 branch scenarios and twelve original Java TC dynamic global transactions')
