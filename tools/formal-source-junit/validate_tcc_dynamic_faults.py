"""Bind dynamic reports and actual TC faults to immutable identity, terminal authority and SQL."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_xa_managed import string,only
from validate_xa_java_faults import frames
from validate_tcc_dynamic import MODES,expected_app,context_versions,INSERT,DELETE
from xa_managed_java_faults import guard

FAULTS=['duplicate','reconnect','report_lost','report_cancel']
def load(path):return json.loads(path.read_bytes())
def cases(versions,modes,faults):
    for version in versions:
        for mode in modes:
            for fault in faults:
                for direction in (['rollback'] if fault.startswith('report_') else ['commit','rollback']):
                    yield version,mode,fault,direction

def check_case(folder,version,mode,fault,direction,alterations=None):
    alterations=alterations or {};prefix='-'.join([version,mode,fault,direction]);case=folder/prefix
    report_fault=fault.startswith('report_');commit=direction=='commit'
    proof=alterations.get('proof',load(case/'observations/proof/original-java-tcc-dynamic-fault.json'))
    expected='Committed' if commit else 'Rollbacked';status=4 if report_fault else 2 if commit else 3
    need(proof['mode']==mode and proof['fault']==fault and proof['branch_type']==1 and proof['lock_key'] is None and proof['resource_id']=='tcc-original-action','Original dynamic fault identity/mode changed')
    need(proof['expected_tc_terminal']==expected and proof['initial_global_status'] in [None,expected] and proof['journal']=='Completed' and proof['status']==status and proof['client_end_requests']==1 and proof['finished_inferred_as_success'] is False,'Unknown global end was inferred or terminal direction changed')
    need(proof['try_committed'] is (not report_fault) and proof['try_future_cancelled'] is (fault=='report_cancel') and proof['report_error_unknown'] is (fault=='report_lost') and proof['original_receipt_retained'] is True,'Unconfirmed report committed Try or original cancellation/error boundary lost')
    need(load(case/'client-command.json')['exit_code']==0 and load(case/'process-cleanup.json')['stopped'],'Actual dynamic fault client/cleanup failed')
    rm,summary,events=frames(case/'RM-frames.jsonl');tm,tm_summary,_=frames(case/'TM-frames.jsonl')
    rm=alterations.get('rm',rm);summary=alterations.get('summary',summary);tm=alterations.get('tm',tm)
    need(summary['mode']==fault and summary['faults']==1 and tm_summary['mode']=='basic','Exactly one actual selected RM fault missing')
    need(all(r['action']=='forward' and r['delivery_count']==1 for r in tm),'TM decision was rewritten')
    wire=[(r['kind'],r['body'],r['direction']) for r in rm]
    register=only(wire,11,'c2s');xid,pos=string(register,2);need(register[pos]==1,'Original registration not TCC');resource,pos=string(register,pos+1);lock,pos=string(register,pos,4);original,pos=string(register,pos,4)
    need(xid==proof['xid'] and resource=='tcc-original-action' and lock is None and original==proof['registered_application_data'] and pos==len(register),'Successful original registration rewritten')
    expected_original={'actionContext':{'useTCCFence':True,'id':1,'amount':7}}
    if mode=='precision':
        need(original=='{"actionContext":{"useTCCFence":true,"id":1,"amount":7,"precise":1.23000000000000000000000000001}}','Precision registration bytes changed')
    else:need(original=='{"actionContext":{"useTCCFence":true,"id":1,"amount":7}}','Original registration values/bytes changed')
    response=only(wire,12,'s2c');branch=int.from_bytes(response[4:12],'big',signed=True)
    need(response[2:4]==b'\x01\x00' and branch>0 and str(branch)==proof['branch_id'],'Original positive branch receipt missing')
    req=next(r for r in rm if r['kind']==11 and r['direction']=='c2s');res=next(r for r in rm if r['kind']==12 and r['direction']=='s2c')
    need((req['connection'],req['request_id'])==(res['connection'],res['request_id']),'Registration correlation changed')
    reports=[r for r in rm if r['kind']==13 and r['direction']=='c2s'];acks=[r for r in rm if r['kind']==14 and r['direction']=='s2c'];apps=[]
    need(len(reports)==len(acks)==(2 if mode=='immediate' and not report_fault else 1),'Report was skipped or replayed after unknown result')
    for index,row in enumerate(reports):
        body=row['body'];value,pos=string(body,2);need(value==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==1,'Report changed original identity/Registered status')
        value,pos=string(body,pos+9);app,pos=string(body,pos,4);need(value is None and body[pos:]==b'\x01','Report optional resource/TCC mode changed')
        expected_app(app,mode,index==len(reports)-1 and not report_fault);apps.append(app)
        matches=[r for r in acks if (r['connection'],r['request_id'])==(row['connection'],row['request_id'])]
        need(len(matches)==1 and matches[0]['body']==b'\x00\x0e\x01\x00' and matches[0]['index']>row['index'],'Report lacks original successful TC response/correlation')
        need(row['action']=='forward' and row['delivery_count']==1,'Original report not forwarded once')
    need(apps[-1]==proof['reported_application_data'],'Original TC callback context not latest complete report')
    callbacks=[r for r in rm if r['kind']==(3 if commit else 5)];replies=[r for r in rm if r['kind']==(4 if commit else 6)]
    need(0<len(callbacks)<=64 and 0<len(replies)<=64 and not any(r['kind'] in ([5,6] if commit else [3,4]) for r in rm),'Missing/unbounded callback or global direction switched')
    correlations=[(r['connection'],r['request_id']) for r in callbacks];need(len(correlations)==len(set(correlations)),'Callback correlation reused')
    for row in callbacks:
        body=row['body'];value,pos=string(body,2);need(row['direction']=='s2c' and value==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==1,'Callback changed original TCC branch')
        value,pos=string(body,pos+9);app,pos=string(body,pos,4);need(value==resource and app==apps[-1] and pos==len(body),'Retry dropped or altered latest context')
    for row in replies:
        body=row['body'];value,pos=string(body,4)
        need(row['direction']=='c2s' and body[2:4]==b'\x01\x00' and value==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8:]==bytes([5 if commit else 8]),'Actual callback terminal reply differs')
        need((row['connection'],row['request_id']) in correlations,'Reply lacks same-session original callback')
    closes=[r for r in events if r['event']=='fault_close']
    if fault=='duplicate':
        injected=[r for r in callbacks if r['action']=='duplicate_original']
        need(len(injected)==1 and injected[0]['delivery_count']==2,'Original callback not duplicated exactly twice')
        target=injected[0];key=(target['connection'],target['request_id'])
        paired=[r for r in replies if (r['connection'],r['request_id'])==key]
        need(len(paired)==2 and [r['action'] for r in paired]==['hold','release_original'] and [r['delivery_count'] for r in paired]==[0,1] and paired[0]['frame_hex']==paired[1]['frame_hex']==paired[1]['released_frame_hex'],'Duplicate terminal replies changed original bytes')
        natural=[r for r in callbacks if r is not target]
        need(all(r['action']=='forward' and r['delivery_count']==1 and r['connection']==target['connection'] and r['body']==target['body'] and target['monotonic_ns']<r['monotonic_ns']<paired[1]['monotonic_ns'] for r in natural),'Additional callback not a bounded original TC retry while first delivery remained unresolved')
        need(all(r['action']=='forward' and r['delivery_count']==1 for r in replies if (r['connection'],r['request_id'])!=key),'Natural TC retry response was altered')
        need(summary['released']==1 and summary['closed']==0 and not closes,'Duplicate scenario closed original connection')
    elif fault=='reconnect':
        need(len(closes)==1 and summary['closed']==1 and summary['connections']>=2 and len(replies)>=2,'Terminal loss/session close/reconnect missing')
        need(replies[0]['action']=='drop_and_close' and replies[0]['delivery_count']==0 and all(r['action']=='forward' and r['delivery_count']==1 for r in replies[1:]),'Exactly one original terminal reply was not lost')
        old=replies[0]['connection'];need(old==closes[0]['connection'] and all(r['connection']>old for r in replies[1:]),'Acknowledged retries did not move to new RM session')
        need(all(r['action']=='forward' and r['delivery_count']==1 and r['body']==callbacks[0]['body'] for r in callbacks),'TC retry changed original callback')
        need(all(r['monotonic_ns']<closes[0]['monotonic_ns'] for r in callbacks if r['connection']==old),'Old callback occurred after actual close')
        for row in replies[1:]:
            need(any(r['kind']==103 and r['connection']==row['connection'] for r in rm) and any(r['kind']==104 and r['connection']==row['connection'] and r['body'][2:4]==b'\x01\x00' for r in rm),'New RM session not registered')
    else:
        need(direction=='rollback' and len(closes)==1 and summary['closed']==1 and summary['connections']>=2,'Report fault did not close original session before empty rollback')
        need(acks[0]['action']=='drop_and_close' and acks[0]['delivery_count']==0 and all(r['action']=='forward' and r['delivery_count']==1 for r in callbacks+replies),'Original successful report response was not lost exactly once')
        reconnected=alterations.get('reconnected',load(case/'reconnected-before-end.json'))
        ack104=[r for r in rm if r['kind']==104 and r['connection']==reconnected['new_connection'] and r['index']==reconnected['registration_ack_index']]
        need(reconnected['closed_connection']==closes[0]['connection'] and reconnected['fault_close_index']==closes[0]['index'] and reconnected['new_connection']>closes[0]['connection'] and len(ack104)==1 and ack104[0]['body'][2:4]==b'\x01\x00','Report cancellation/loss lacked original closed session and fresh registration')
        need(all(r['connection']>=reconnected['new_connection'] for r in callbacks+replies),'Empty Cancel used old report session')
    if not report_fault:need(all(r['action']=='forward' and r['delivery_count']==1 for r in acks),'Report fault injected into callback-only scenario')
    for callback in callbacks:
        matched=[r for r in replies if (r['connection'],r['request_id'])==(callback['connection'],callback['request_id'])]
        if fault=='reconnect' and callback['connection']==replies[0]['connection']:
            count=int(callback['request_id']==replies[0]['request_id'])
        else:count=2 if fault=='duplicate' and callback['action']=='duplicate_original' else 1
        need(len(matched)==count,'Original callback response missing or duplicated outside injected pair')
    affected=callbacks+replies+(acks if report_fault else [])
    need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm if r not in affected),'Unrelated RM traffic altered')
    tm_wire=[(r['kind'],r['body'],r['direction']) for r in tm];only(tm_wire,1,'c2s');begin=only(tm_wire,2,'s2c')
    need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'Original global Begin changed')
    need(string(only(tm_wire,7 if commit else 9,'c2s'),2)[0]==xid and not any(r['kind'] in [15,16] for r in tm),'Global End repeated or status query used as outcome')
    end_request=next(r for r in tm if r['kind']==(7 if commit else 9) and r['direction']=='c2s')
    end_replies=[r for r in tm if r['kind']==(8 if commit else 10) and r['direction']=='s2c']
    need(len(end_replies)<=1 and all((r['connection'],r['request_id'])==(end_request['connection'],end_request['request_id']) for r in end_replies),'Global End response correlation changed')
    if proof['initial_global_status']==expected:
        need(len(end_replies)==1 and end_replies[0]['body'][2:4]==b'\x01\x00' and end_replies[0]['body'][-1]==(9 if commit else 11),'Initial resolved TM result lacks actual explicit success')
    if report_fault:
        need(end_request['monotonic_ns']>ack104[0]['monotonic_ns'],'TM Rollback sent before report recovery observation')
    branch_log=('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)
    global_log=('Committing global transaction is successfully done, xid = ' if commit else 'Rollback global transaction successfully, xid = ')+xid+'.'
    need(load(case/'original-tc-terminal.json')==dict(xid=xid,branch_id=str(branch),direction=direction,original_branch_success_log=branch_log,original_global_success_log=global_log,Finished_used_as_success=False),'Original TC terminal observation changed')
    logs=alterations.get('tc_logs',(case/'tc-stdout.log').read_bytes());need(branch_log.encode() in logs and global_log.encode() in logs,'Original TC terminal logs missing')
    def fact(name):return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    context_versions(fact('independent-report-context'),xid,branch,original,apps)
    need(fact('final-context')==fact('independent-report-context'),'Context versions changed/deleted during phase two')
    tc_row=(xid.encode().hex().upper()+'\t'+str(branch)+'\tTCC\t1\t'+apps[-1].encode().hex().upper()).encode()
    need(fact('independent-report-tc')==tc_row and fact('independent-report-business')==(b'' if report_fault else b'1\t7') and fact('before-end-fence')==(b'0' if report_fault else b'1'),'Original TC report/actual Try boundary contradicted')
    if fault=='report_cancel':
        held=alterations.get('held',load(case/'report-held.json'))
        need(held['frame_hex']==acks[0]['frame_hex'] and held['connection']==acks[0]['connection'] and held['request_id']==acks[0]['request_id'] and held['xid']==xid and held['branch_id']==str(branch) and held['application_data']==apps[0] and held['request_frame_sha256']==hashlib.sha256(bytes.fromhex(reports[0]['frame_hex'])).hexdigest(),'Cancellation did not hold original matching successful TC response')
        need(reports[0]['monotonic_ns']<held['monotonic_ns']<acks[0]['monotonic_ns'],'Held original response not observed before cancellation release')
        need(fact('held-tc')==tc_row and fact('held-context')==fact('independent-report-context') and not fact('held-business') and fact('held-fence')==b'0','Cancellation boundary did not observe TC new context and uncommitted Try')
    uuid=fact('independent-report-context').splitlines()[0].split(b'\t')[6].decode()
    fence=(xid.encode().hex().upper()+'\t'+str(branch)+'\t'+resource.encode().hex().upper()+'\t'+original.encode().hex().upper()+'\t'+uuid+'\t2\t'+str(status)).encode()
    need(fact('final-fence')==fence and fact('fence-protocol')==b'2' and fact('independent-state')==(xid.encode().hex().upper()+'\t'+str(branch)+'\t'+str(status)).encode(),'Original identity/Fence protocol/terminal differs')
    value=14 if mode=='precision' else 18;business=([[1,value]] if commit else []);dml=[INSERT] if report_fault else [INSERT,b'Execute\tUPDATE tcc_managed SET v='+str(value).encode()+b' WHERE id=1' if commit else DELETE]
    need(proof['business']==business and fact('independent-business')==(('1\t'+str(value)).encode() if commit else b'') and fact('business-dml').splitlines()==dml,'Unconfirmed Try committed, empty Cancel invoked business, or terminal DML replayed')
    need(fact('independent-tc')==b'0\t0\t0' and not fact('independent-prepared') and fact('native-commands')==b'Query\tXA RECOVER CONVERT XID','TC cleanup or native mode differs')

def verify(folder,digest):
    catalog(folder,digest);sources(folder);result=load(folder/'result.json')
    need(result['Rust_versions']==['1.99','1.85'] and result['modes']==MODES and result['faults']==FAULTS and result['actual_global_transactions']==36 and result['Java_pin']==guard.reg.PIN and result['Java_jar_sha256']==guard.reg.JAR_SHA and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False,'Dynamic original TC fault matrix/pin/scope differs')
    original=subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java'])
    need((folder/'original-DefaultCore.java').read_bytes()==original,'Terminal source not unchanged pinned TC')
    need(load(folder/'cleanup.json')['owned_container_removed'],'Owned dynamic fault database remains')
    binding=load(folder/'auth-binding.json')
    allowed={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'}
    need(set(binding['runtime'])==allowed,'Fixture overlay replaced transaction coordinator classes')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'Auth-only fixture source changed')
    for args in cases(result['Rust_versions'],result['modes'],result['faults']):check_case(folder,*args)

def weakenings(folder):
    rejected=0;overlaps=0
    for version,mode,fault,direction in cases(['1.99','1.85'],MODES,FAULTS):
        args=(folder,version,mode,fault,direction);prefix='-'.join([version,mode,fault,direction]);case=folder/prefix
        proof=load(case/'observations/proof/original-java-tcc-dynamic-fault.json');variants=[]
        for field,value in [('finished_inferred_as_success',True),('client_end_requests',2),('branch_type',2),('journal','Pending'),('status',0),('business',[[99,99]]),('try_committed',not proof['try_committed']),('try_future_cancelled',not proof['try_future_cancelled']),('report_error_unknown',not proof['report_error_unknown']),('original_receipt_retained',False),('initial_global_status','Finished'),('registered_application_data','{}'),('reported_application_data','{}')]:
            changed=copy.deepcopy(proof);changed[field]=value;variants.append(dict(proof=changed))
        for fact in ['independent-report-tc','independent-report-context','independent-report-business','before-end-fence','final-context','final-fence','business-dml','independent-business','independent-tc']:variants.append(dict(facts={fact:b'changed'}))
        rm,summary,_=frames(case/'RM-frames.jsonl')
        variants.append(dict(rm=[r for r in copy.deepcopy(rm) if r['kind']!=13]))
        changed=copy.deepcopy(rm);row=next(r for r in changed if r['kind']==13);_,pos=string(row['body'],2);body=bytearray(row['body']);body[pos+8]=2;row['body']=bytes(body);variants.append(dict(rm=changed))
        changed=copy.deepcopy(rm);row=next(r for r in changed if r['kind']==(3 if direction=='commit' else 5));row['body']=row['body'].replace(b'"id":1',b'"id":2');variants.append(dict(rm=changed))
        changed=copy.deepcopy(rm);row=next(r for r in changed if r['kind']==(4 if direction=='commit' else 6));row['body']=row['body'][:-1]+b'\x0a';variants.append(dict(rm=changed))
        changed=copy.deepcopy(summary);changed['faults']=0;variants.append(dict(summary=changed));variants.append(dict(tc_logs=b'Finished'))
        if fault.startswith('report_'):
            changed=load(case/'reconnected-before-end.json');changed['new_connection']=1;variants.append(dict(reconnected=changed))
        if fault=='report_cancel':
            for key,value in [('frame_hex','00'),('request_frame_sha256','0'*64)]:
                changed=load(case/'report-held.json');changed[key]=value;variants.append(dict(held=changed))
            for fact in ['held-context','held-tc','held-fence','held-business']:variants.append(dict(facts={fact:b'changed'}))
        if fault=='duplicate':
            target=next(r for r in rm if r['kind']==(3 if direction=='commit' else 5) and r['action']=='duplicate_original')
            natural=[r for r in rm if r['kind']==target['kind'] and r['action']=='forward']
            if natural:
                overlaps+=1
                for field,value in [('connection',target['connection']+1),('request_id',target['request_id'])]:
                    changed=copy.deepcopy(rm);next(r for r in changed if r['index']==natural[0]['index'])[field]=value;variants.append(dict(rm=changed))
        if fault=='reconnect':
            old=[r for r in rm if r['kind']==(3 if direction=='commit' else 5) and r['connection']==1]
            if len(old)>1:
                overlaps+=1
                for field,value in [('monotonic_ns',2**63),('request_id',old[0]['request_id'])]:
                    changed=copy.deepcopy(rm);next(r for r in changed if r['index']==old[1]['index'])[field]=value;variants.append(dict(rm=changed))
        for change in variants:
            try:check_case(*args,change)
            except ValueError:rejected+=1
            else:raise RuntimeError('Dynamic fault evidence weakening accepted')
    need(rejected==1056+2*overlaps,'Dynamic fault weakening matrix incomplete');return rejected

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('catalog_sha256');parser.add_argument('--weakenings',action='store_true');args=parser.parse_args()
    verify(args.folder,args.catalog_sha256)
    if args.weakenings:print('Rejected dynamic original TC fault evidence weakenings:',weakenings(args.folder))
    print('Verified 36 original Java TC dynamic fault transactions; full TCC and whole-plan gates remain open')
