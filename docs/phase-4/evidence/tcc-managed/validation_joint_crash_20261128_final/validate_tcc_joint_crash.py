"""Independent original TC/Rust joint-crash proof: raw frames, OS handles and physical state."""
import argparse,copy,hashlib,json,struct,zlib,subprocess,itertools,re
from pathlib import Path
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog,sources
from validate_xa_managed import string,only
from validate_tcc_dynamic import expected_app,context_versions
from xa_managed_java_faults import guard

MODES=['delayed','immediate','precision'];DATABASES=['none','restart','crash'];VERSIONS=['1.99','1.85'];DIRECTIONS=['commit','rollback']
def load(p):return json.loads(p.read_bytes())
def frames(path):
    events=[json.loads(line) for line in path.read_bytes().splitlines()];rows=[]
    for event in events:
        if event['event']!='frame':continue
        row=event.copy();raw=bytes.fromhex(row['frame_hex']);head=int.from_bytes(raw[7:9],'big');body=raw[head:]
        need(raw[:3]==b'\xda\xda\x01' and int.from_bytes(raw[3:7],'big')==len(raw) and 16<=head<=len(raw)<=65536,'Malformed original frame')
        need(row['request_type']==raw[9] and row['request_id']==int.from_bytes(raw[12:16],'big',signed=True),'Frame correlation rewritten')
        if raw[9] in [3,4]:need(not body and row['kind'] is None,'Invalid original heartbeat')
        else:need(len(body)>=2 and int.from_bytes(body[:2],'big')==row['kind'],'Original frame kind differs')
        row['body']=body;rows.append(row)
    summary=[r for r in events if r['event']=='summary'];need(len(summary)==1 and summary[0]['succeeded'] is True and not summary[0]['errors'] and not any(r['event']=='error' for r in events),'Observer incomplete')
    return rows,summary[0],events

def journal(data,xid,branch,original,commit):
    need(data[:8]==b'SRMJNL01','Original journal header differs');pos=8;states=[]
    resource=b'tcc-original-action';payload=struct.pack('>HH',3 if commit else 5,len(xid))+xid.encode()+struct.pack('>qBH',branch,1,len(resource))+resource+struct.pack('>I',len(original.encode()))+original.encode()
    while pos<len(data):
        size=int.from_bytes(data[pos:pos+4],'big');pos+=4;record=data[pos:pos+size];pos+=size
        need(size>=13 and len(record)==size and zlib.crc32(record[:-4])==int.from_bytes(record[-4:],'big'),'Journal CRC/truncation differs')
        need(int.from_bytes(record[:8],'big')==len(states)+1 and record[9:-4]==payload,'Journal identity changed to report bytes')
        states.append(record[8])
    need(states==[1,3],'Original journal did not persist Pending then Completed once')

def check_case(folder,version,mode,database,direction,changes=None):
    changes=changes or {};prefix='-'.join([version,mode,database,direction]);case=folder/prefix;commit=direction=='commit'
    proof=changes.get('proof',load(case/'recovery-observations/proof/original-java-tcc-joint-recovery.json'));before=load(case/'writer-observations/proof/dynamic-before-end.json')
    xid=before['xid'];branch=int(before['branch_id']);original=before['registered_application_data'];latest=before['reported_application_data']
    expected_original='{"actionContext":{"useTCCFence":true,"id":1,"amount":7'+(',"precise":1.23000000000000000000000000001' if mode=='precision' else '')+'}}'
    need(original==expected_original and before['mode']==mode and before['fault']=='joint_crash' and before['try_committed'] is True and before['report_error_unknown'] is before['try_future_cancelled'] is False,'Original successful Try boundary differs')
    expected={'mode':mode,'direction':direction,'xid':xid,'branch_id':str(branch),'registered_application_data':original,'reported_application_data':latest,'journal':'Completed','status':2 if commit else 3,'protocol':2,'business':([[1,14 if mode=='precision' else 18]] if commit else []),'new_global_end_requests':0,'new_business_registrations':0,'new_reports':0,'finished_inferred_as_success':False}
    for k,v in expected.items():need(type(proof[k]) is type(v) and proof[k]==v,'Recovery proof changed: '+k)
    def record(name):return changes.get(name,load(case/(name+'.json')))
    def fact(name):return changes.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    writer,tc=record('writer-kill'),record('tc-kill');auth=record('joint-kill');new_tc=record('tc-after-live');command=record('recovery-command')
    for item in [writer,tc]:need(type(item['pid']) is int and item['pid']>0 and item['before']==dict(alive=True,exit_code=259) and item['after']['alive'] is False and type(item['after']['exit_code']) is int and item['after']['exit_code']!=0,'Original owned process was not observed live then forcibly exited')
    need(all(type(auth[k]) is int and auth[k]>0 for k in ['writer_pid','tc_pid','monotonic_ns']),'Invalid controlled-exit authorization types')
    need(auth['phase']=='joint-kill' and auth['writer_pid']==writer['pid']==before['rust_process_id']==proof['writer_pid'] and auth['tc_pid']==tc['pid'] and auth['monotonic_ns']<writer['monotonic_ns']<tc['monotonic_ns'],'Kill authorization/OS identities or order changed')
    need(new_tc['alive'] is True and new_tc['exit_code']==259 and new_tc['pid']==command['tc_pid'] and new_tc['monotonic_ns']>tc['monotonic_ns'],'New TC was not observed live after old TC exit')
    need(type(command['pid']) is int and command['pid']==proof['rust_process_id'] and len({writer['pid'],tc['pid'],new_tc['pid'],command['pid']})==4 and command['exit_code']==0 and command['argv'][1:]==['original_java_tc_recovers_joint_crash_tcc_branch','--ignored','--exact','--nocapture'],'Recovery was not a fresh RM-only native process')
    need(b'test result: ok. 1 passed;' in (case/'recovery-stdout.log').read_bytes() and len(command['artifact_sha256'])==64,'Native recovery did not execute')
    old,old_summary,old_events=frames(case/'RM-before-frames.jsonl');tm,tm_summary,tm_events=frames(case/'TM-before-frames.jsonl');new,new_summary,new_events=frames(case/'RM-after-frames.jsonl')
    old=changes.get('old',old);tm=changes.get('tm',tm);new=changes.get('new',new)
    need(old_summary['mode']=='joint_crash' and old_summary['faults']==1 and old_summary['closed']==old_summary['released']==0 and tm_summary['mode']==new_summary['mode']=='basic','Wrong observer phase')
    need(any(r['event']=='joint_crash_rule_installed' for r in old_events),'Joint hold rule absent')
    digest=hashlib.sha256((case/'joint-kill.json').read_bytes()).hexdigest()
    for event in changes.get('reset-events',[r for r in old_events+tm_events if r['event']=='expected_peer_reset']):
        need(event['error_type'] in ['ConnectionResetError','ConnectionAbortedError'] and event['authorization_sha256']==digest and auth['monotonic_ns']<=event['monotonic_ns']<=auth['monotonic_ns']+10_000_000_000,'Unrelated observer reset counted as authorized exit')
    need(not any(r['event']=='expected_peer_reset' for r in new_events),'Recovery connection reset was silently accepted')
    ready={}
    for name,events in [('old',old_events),('tm',tm_events),('new',new_events)]:
        selected=[r for r in events if r['event']=='ready'];need(len(selected)==1,'Observer endpoint declaration missing');ready[name]=selected[0]
    ready=changes.get('ready',ready)
    service_port=int(re.search(r'service-port: (\d+)',(case/'application.yml').read_text(encoding='utf-8')).group(1))
    need(ready['old']['upstream_port']==ready['tm']['upstream_port']==ready['new']['upstream_port']==service_port and ready['old']['listen_port']==ready['new']['listen_port'] and ready['old']['role']==ready['new']['role']=='RM' and ready['tm']['role']=='TM','Recovered TC/RM endpoints or roles changed')
    wire=[(r['kind'],r['body'],r['direction']) for r in old]
    registration=only(wire,11,'c2s');v,pos=string(registration,2);need(v==xid and registration[pos]==1,'Original branch registration changed')
    resource,pos=string(registration,pos+1);lock,pos=string(registration,pos,4);app,pos=string(registration,pos,4)
    need(resource=='tcc-original-action' and lock is None and app==original and pos==len(registration),'Original receipt data/resource changed')
    receipt=only(wire,12,'s2c');need(receipt[2:4]==b'\x01\x00' and int.from_bytes(receipt[4:12],'big',signed=True)==branch and branch>0,'Original successful receipt missing')
    requests=[r for r in old if r['kind']==11 and r['direction']=='c2s'];replies=[r for r in old if r['kind']==12 and r['direction']=='s2c'];need((requests[0]['connection'],requests[0]['request_id'])==(replies[0]['connection'],replies[0]['request_id']),'Registration correlation changed')
    reports=[r for r in old if r['kind']==13 and r['direction']=='c2s'];acks=[r for r in old if r['kind']==14 and r['direction']=='s2c'];need(len(reports)==len(acks)==(2 if mode=='immediate' else 1),'Report skipped/replayed');apps=[]
    for i,row in enumerate(reports):
        body=row['body'];v,pos=string(body,2);need(v==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==1,'Report changed identity/Registered')
        v,pos=string(body,pos+9);app,pos=string(body,pos,4);need(v is None and body[pos:]==b'\x01','Report changed absent resource/TCC');expected_app(app,mode,i==len(reports)-1);apps.append(app)
        matched=[r for r in acks if (r['connection'],r['request_id'])==(row['connection'],row['request_id'])];need(len(matched)==1 and matched[0]['body']==b'\x00\x0e\x01\x00' and matched[0]['index']>row['index'],'Original report success/correlation missing')
    need(apps[-1]==latest,'Latest report differs from recovery observation')
    held=record('held-callback');old_callbacks=[r for r in old if r['kind'] in [3,5]]
    need(old_callbacks and not any(r['kind'] in [4,6] for r in old),'Undelivered callback produced a response')
    def callback(row):
        body=row['body'];v,pos=string(body,2);need(row['kind']==(3 if commit else 5) and row['direction']=='s2c' and v==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8]==1,'Callback switched original TCC identity/direction');v,pos=string(body,pos+9);app,pos=string(body,pos,4);need(v==resource and app==latest and pos==len(body),'Callback lost complete latest report')
    for row in old_callbacks:callback(row);need(row['action']=='hold_for_joint_crash' and row['delivery_count']==0 and row['monotonic_ns']<auth['monotonic_ns'],'Callback was delivered before old writer died')
    need(held['frame_hex']==old_callbacks[0]['frame_hex'] and held['connection']==old_callbacks[0]['connection'] and held['request_id']==old_callbacks[0]['request_id'] and held['monotonic_ns']<=old_callbacks[0]['monotonic_ns'] and held['kind']==(3 if commit else 5) and held['xid']==xid and held['branch_id']==str(branch),'Held cut not original TC callback')
    need(all(r['action']=='forward' and r['delivery_count']==1 for r in old if r not in old_callbacks) and all(r['action']=='forward' and r['delivery_count']==1 for r in tm+new),'Unrelated frame altered')
    tm_wire=[(r['kind'],r['body'],r['direction']) for r in tm];only(tm_wire,1,'c2s');begin=only(tm_wire,2,'s2c');need(begin[2:4]==b'\x01\x00' and string(begin,4)[0]==xid,'Original TM Begin differs');end=only(tm_wire,7 if commit else 9,'c2s');need(string(end,2)[0]==xid and not any(r['kind'] in [15,16,9 if commit else 7] for r in tm),'Global End repeated/switched or status query used')
    need(not any(r['kind'] in [1,2,7,8,9,10,11,12,13,14,15,16] for r in new),'Recovery resent Begin/End/branch/report')
    reg=next((r for r in new if r['kind']==103 and r['direction']=='c2s'),None);reg_ack=next((r for r in new if r['kind']==104 and r['direction']=='s2c'),None);need(reg is not None and reg_ack is not None and reg_ack['body'][2:4]==b'\x01\x00' and (reg['connection'],reg['request_id'])==(reg_ack['connection'],reg_ack['request_id']),'New RM did not register same resource')
    need(reg['body']==next(r for r in old if r['kind']==103 and r['direction']=='c2s')['body'],'Recovery RM changed resource/registration identity')
    pos=2;registration_fields=[]
    for _ in range(4):v,pos=string(reg['body'],pos);registration_fields.append(v)
    resources,pos=string(reg['body'],pos,4)
    need(registration_fields==['1.1.0','phase0-auth','default_tx_group','phase0-local-only'] and resources==resource and pos==len(reg['body']),'Recovery RM did not register original resource/credential scope')
    callbacks=[r for r in new if r['kind'] in [3,5]];responses=[r for r in new if r['kind'] in [4,6]];need(0<len(callbacks)<=64 and len(callbacks)==len(responses) and len({(r['connection'],r['request_id']) for r in callbacks})==len(callbacks),'New callback response/correlation missing')
    success=0
    for request in callbacks:
        callback(request);need(request['index']>reg_ack['index'] and request['monotonic_ns']>new_tc['monotonic_ns'],'New callback preceded actual TC restart/registration')
        matched=[r for r in responses if (r['connection'],r['request_id'])==(request['connection'],request['request_id'])];need(len(matched)==1,'Response not same-session original retry');response=matched[0];body=response['body'];pos=3
        if body[2]==0:
            msg,pos=string(body,pos);need(body[pos]==0 and msg is not None,'Failed callback prefix malformed');pos+=1;status=6 if commit else 9
        else:need(body[2:4]==b'\x01\x00','Unknown callback result');pos=4;status=5 if commit else 8;success+=1
        v,pos=string(body,pos);need(response['kind']==(4 if commit else 6) and response['direction']=='c2s' and response['index']>request['index'] and v==xid and int.from_bytes(body[pos:pos+8],'big',signed=True)==branch and body[pos+8:]==bytes([status]),'Actual recovery callback result/identity differs')
    need(success>=1,'No actual terminal success frame')
    for name,pid in [('tc-stdout.log',tc['pid']),('tc-after-stdout.log',new_tc['pid'])]:
        need(('with PID '+str(pid)+' ').encode() in (case/name).read_bytes(),'TC startup log not bound to observed JVM PID')
    cut=fact('state-cut');need(cut==fact('state-before-database')==fact('state-after-database'),'Original session/decision changed while both owners dead')
    rows=cut.splitlines();need(len(rows)==5 and len(rows[0])==36,'Original cut state shape differs');uuid=rows[0].decode();hx=xid.encode().hex().upper();hexapp=original.encode().hex().upper();hexlatest=latest.encode().hex().upper()
    def fence(status):return '\t'.join([hx,str(branch),resource.encode().hex().upper(),hexapp,uuid.encode().hex().upper(),'2',str(status)]).encode()
    need(rows[1]==(hx+'\t'+('2' if commit else '4')).encode() and rows[2]=='\t'.join([hx,str(branch),'TCC','1',hexlatest]).encode() and rows[3]==fence(1) and rows[4]==b'1\t7','Original durable TC decision/report/Try not observed')
    context=fact('context-cut');need(context==fact('context-after-database')==fact('context-final'),'Report versions changed/deleted during joint recovery');context_versions(context,xid,branch,original,apps);need(bytes.fromhex(context.splitlines()[0].split(b'\t')[6].decode()).decode()==uuid,'Context instance differs')
    final=[uuid.encode(),fence(2 if commit else 3)]+([('1\t'+str(14 if mode=='precision' else 18)).encode()] if commit else [])
    need(fact('state-final').splitlines()==final and fact('tc-final')==b'0\t0\t0','Final original Fence/business/TC cleanup differs')
    need(fact('business-dml-before')==b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(1,7)' and fact('business-dml-after')==((b'Execute\tUPDATE tcc_managed SET v='+str(14 if mode=='precision' else 18).encode()+b' WHERE id=1') if commit else b'Execute\tDELETE FROM tcc_managed WHERE id=1'),'Business replayed/skipped or original decision reversed')
    need(changes.get('journal-cut',(case/'journal-at-cut.bin').read_bytes())==b'SRMJNL01','Undelivered old callback changed journal')
    journal(changes.get('journal-final',(case/'writer-observations/java-journal/branches.log').read_bytes()),xid,branch,original,commit)
    terminal=load(case/'original-tc-terminal.json');branch_log=('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch);global_log=('Committing global transaction is successfully done, xid = ' if commit else 'Rollback global transaction successfully, xid = ')+xid+'.'
    need(terminal==dict(global_success_log=global_log,branch_success_log=branch_log,original_global_end_requests=1,new_global_end_requests=0,Finished_used_as_success=False),'Original TC terminal observation changed')
    logs=changes.get('logs',(case/'tc-after-stdout.log').read_bytes());need(branch_log.encode() in logs and global_log.encode() in logs,'Restarted TC explicit terminal logs missing')
    need(load(case/'process-cleanup.json')['stopped'] is True,'Owned joint processes remain')
    for name in ['tc-before-class-load.log','tc-after-class-load.log']:
        need(any(b'org.apache.seata.server.coordinator.DefaultCore source:' in line and b'seata-server.jar' in line for line in (case/name).read_bytes().splitlines()),'Original coordinator was not loaded from server JAR')
    if database!='none':
        stopped=load(folder/(prefix+'-database-stopped-inspect.stdout'))[0];final_inspect=load(folder/'inspect.stdout')[0]
        need(stopped['Id']==final_inspect['Id'] and stopped['Mounts']==final_inspect['Mounts'] and stopped['Image']==guard.IMAGE and stopped['Config']['Labels']==final_inspect['Config']['Labels'] and not stopped['State']['Running'] and stopped['State']['ExitCode']==(137 if database=='crash' else 0),'Database was not same-data owned restart/crash')
    return proof

def verify(folder,digest):
    catalog(folder,digest);sources(folder);result=load(folder/'result.json')
    need(result['status']=='ORIGINAL_JAVA_TC_TCC_JOINT_CRASH_RECOVERY_VERIFIED' and result['Rust_versions']==VERSIONS and result['modes']==MODES and len(result['database_modes'])==1 and result['database_modes'][0] in DATABASES and result['directions']==DIRECTIONS and result['actual_global_transactions']==12 and result['Java_pin']==guard.reg.PIN and result['Java_jar_sha256']==guard.reg.JAR_SHA and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False,'Joint scope/pin changed')
    original=subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java']);need((folder/'original-DefaultCore.java').read_bytes()==original,'Pinned TC coordinator source changed')
    binding=load(folder/'auth-binding.json');need(set(binding['runtime'])=={'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'},'Overlay replaced coordinator')
    for source,digest in binding['sources'].items():need(sha(Path(source))==digest,'Auth-only fixture source changed')
    need(load(folder/'cleanup.json')['owned_container_removed'],'Owned database remains')
    inspected=load(folder/'inspect.stdout')[0];created=(folder/'create.stdout').read_bytes().strip().decode();create_command=next(r for r in load(folder/'commands.json') if r['label']=='create')['argv'];label=create_command[create_command.index('--label')+1]
    need(inspected['Id']==created and inspected['Image']==guard.IMAGE and inspected['State']['Running'] is True and label=='seata.tcc.owner='+inspected['Config']['Labels']['seata.tcc.owner'],'Final original owned database identity/image differs')
    need(all(r['exit_code']==(1 if r['label']=='confirm-removed' else 0) for r in load(folder/'commands.json')),'Native/schema/database command failed')
    for version,mode,database,direction in itertools.product(VERSIONS,MODES,result['database_modes'],DIRECTIONS):check_case(folder,version,mode,database,direction)

def weakenings(folder):
    count=0;database=load(folder/'result.json')['database_modes'][0]
    for version,mode,direction in itertools.product(VERSIONS,MODES,DIRECTIONS):
        case=folder/'-'.join([version,mode,database,direction]);proof=load(case/'recovery-observations/proof/original-java-tcc-joint-recovery.json');variants=[]
        for key,value in [('journal','Pending'),('status',0),('protocol',1),('business',[[99,99]]),('registered_application_data','{}'),('reported_application_data','{}'),('new_global_end_requests',1),('new_business_registrations',1),('new_reports',1),('finished_inferred_as_success',True),('writer_pid',1),('direction','other')]:
            changed=copy.deepcopy(proof);changed[key]=value;variants.append(dict(proof=changed))
        for name in ['state-cut','state-before-database','state-after-database','state-final','context-cut','context-after-database','context-final','business-dml-before','business-dml-after','tc-final']:variants.append(dict(facts={name:b'changed'}))
        for name in ['writer-kill','tc-kill']:
            changed=load(case/(name+'.json'));changed['after']['alive']=True;variants.append({name:changed})
        changed=load(case/'tc-after-live.json');changed['pid']=load(case/'tc-kill.json')['pid'];variants.append({'tc-after-live':changed})
        variants.extend([{'journal-cut':b'SRMJNL01bad'},{'journal-final':b'SRMJNL01'},{'logs':b'Finished'}])
        old,_,_=frames(case/'RM-before-frames.jsonl');new,_,_=frames(case/'RM-after-frames.jsonl');tm,_,_=frames(case/'TM-before-frames.jsonl')
        changed=copy.deepcopy(old);next(r for r in changed if r['kind'] in [3,5])['delivery_count']=1;variants.append(dict(old=changed))
        changed=copy.deepcopy(new);row=next(r for r in changed if r['kind'] in [3,5]);row['body']=row['body'].replace(b'"id":1',b'"id":2');variants.append(dict(new=changed))
        changed=copy.deepcopy(new);row=next(r for r in changed if r['kind'] in [4,6]);row['body']=row['body'][:-1]+b'\x0a';variants.append(dict(new=changed))
        variants.append(dict(tm=[r for r in tm if r['kind'] not in [7,9]]));variants.append(dict(new=[r for r in new if r['kind'] not in [4,6]]))
        for name in ['writer-kill','tc-kill']:
            changed=load(case/(name+'.json'));changed['after']['exit_code']=0;variants.append({name:changed})
        auth=load(case/'joint-kill.json')
        variants.append({'reset-events':[dict(error_type='ConnectionResetError',authorization_sha256='0'*64,monotonic_ns=auth['monotonic_ns'])]})
        changed=copy.deepcopy(auth);changed['monotonic_ns']=1;variants.append({'joint-kill':changed})
        changed=load(case/'held-callback.json');changed['frame_hex']='00';variants.append({'held-callback':changed})
        variants.append(dict(new=[r for r in new if r['kind']!=104]))
        changed=load(case/'recovery-command.json');changed['argv'][1]='another_test';variants.append({'recovery-command':changed})
        prefix='-'.join([version,mode,database,direction]);raw=(folder/(prefix+'-context-cut.stdout')).read_bytes().strip().replace(b'\r',b'');lines=raw.splitlines();fields=lines[-1].split(b'\t');fields[-1]=b'0'*64;lines[-1]=b'\t'.join(fields);bad=b'\n'.join(lines)
        variants.append(dict(facts={k:bad for k in ['context-cut','context-after-database','context-final']}))
        bad=bytearray((case/'writer-observations/java-journal/branches.log').read_bytes());bad[-1]^=1;variants.append({'journal-final':bytes(bad)})
        changed=copy.deepcopy(new);row=next(r for r in changed if r['kind']==103);row['body']=row['body'].replace(b'tcc-original-action',b'tcc-original-actioN');variants.append(dict(new=changed))
        endpoints={}
        for name,file in [('old','RM-before-frames.jsonl'),('tm','TM-before-frames.jsonl'),('new','RM-after-frames.jsonl')]:endpoints[name]=next(r for r in frames(case/file)[2] if r['event']=='ready').copy()
        endpoints['new']['upstream_port']+=1;variants.append(dict(ready=endpoints))
        for change in variants:
            try:check_case(folder,version,mode,database,direction,change)
            except ValueError:count+=1
            else:raise RuntimeError('Joint-crash evidence weakening accepted')
    need(count==528,'Joint weakening matrix incomplete');return count

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('sha256');p.add_argument('--weakenings',action='store_true');a=p.parse_args();verify(a.folder,a.sha256)
    if a.weakenings:print('Rejected joint-crash evidence weakenings:',weakenings(a.folder))
    print('Verified twelve original TC/Rust joint-crash transactions; full TCC and whole-plan gates remain open')
