"""Owned pinned Java TC and raw loss observers; no production TC acceptance claim."""
import importlib.util
import copy
import json
import os
from pathlib import Path
import re
import socket
import struct
import sys
import zipfile
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('callback_java_process_support',ROOT/'tools/formal-transport-loss/live.py')
support=importlib.util.module_from_spec(spec);spec.loader.exec_module(support)
reg=support.reg
def need(value,reason):
    if not value:raise ValueError(reason)

def inputs():
    return {Path(support.__file__),Path(reg.__file__),reg.ORACLE/'server/target/seata-server.jar',reg.HISTORICAL,reg.FIXTURE/'org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class',reg.FIXTURE/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler',ROOT/'tools/java-tc-integration/config/application.yml',ROOT/'tools/formal-rm-java/validate.py'}

class Runtime:
    def __init__(self,out,work,env):
        self.out=out/'java-tc';self.out.mkdir();self.work=work/'java-tc';self.work.mkdir();self.env=env;self.processes=[];self.stop_file=self.work/'stop-proxy';self.closed=False
        try:
            jar=reg.ORACLE/'server/target/seata-server.jar';need(support.sha(jar)==reg.JAR_SHA,'Pinned Java JAR differs');need(support.command(['git','-C',reg.ORACLE,'rev-parse','HEAD'],env).decode().strip()==reg.PIN,'Pinned Java source differs')
            historical=json.loads(reg.HISTORICAL.read_bytes());extension=self.work/'fixture'
            for leaf,key in [('org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class','extension_class_sha256'),('META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','extension_service_sha256')]:
                p=reg.FIXTURE/leaf;need(support.sha(p)==historical[key],'Runtime auth SPI differs')
                if leaf.endswith('.class'):support.put(extension/leaf,p.read_bytes())
            # 新测试 SPI 仅替换单实例资源匹配；父类与依赖逐字节取自固定原 JAR。
            classes=self.work/'oracle-classes';libraries=self.work/'oracle-libraries';classpath=[classes,extension];extracted=[]
            with zipfile.ZipFile(jar) as archive:
                for name in archive.namelist():
                    if name.startswith('BOOT-INF/classes/') and name.endswith('.class'):
                        target=classes/name[len('BOOT-INF/classes/'):];support.put(target,archive.read(name));extracted.append(dict(member=name,sha256=support.sha(target)))
                    elif name.startswith('BOOT-INF/lib/') and Path(name).name.startswith(('seata-common-','seata-core-','spring-core-','slf4j-api-')) and name.endswith('.jar'):
                        target=libraries/Path(name).name;support.put(target,archive.read(name));classpath.append(target);extracted.append(dict(member=name,sha256=support.sha(target)))
            need(len(classpath)>=6,'Original Java compile dependencies missing')
            support.command([reg.JAVA.parent/'javac.exe','-encoding','UTF-8','-cp',os.pathsep.join(str(p) for p in classpath),'-d',extension,HERE/'CallbackOwnershipAuthHandler.java'],env,self.out/'auth-compile.log')
            support.put(extension/'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org.apache.seata.server.auth.CallbackOwnershipAuthHandler\n')
            compiled=extension/'org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class';support.put(self.out/'CallbackOwnershipAuthHandler.class',compiled.read_bytes())
            resource='jdbc:mysql://127.0.0.1:'+env['SEATA_MYSQL_PORT']+'/'+env['SEATA_MYSQL_DATABASE']
            support.save(self.out/'auth-binding.json',dict(resource=resource,compiled_sha256=support.sha(compiled),source_sha256=support.sha(HERE/'CallbackOwnershipAuthHandler.java'),original_identity_class_sha256=historical['extension_class_sha256'],original_JAR_members=extracted,production_auth_qualified=False))
            for port in [38091,38092,38093]:
                with socket.socket() as s:s.bind(('127.0.0.1',port))
            config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text('utf-8').replace('18091','38091').replace('enable-check-auth: false','enable-check-auth: true');support.put(self.work/'application.yml',config);support.put(self.out/'application.yml',config)
            self.env=env|dict(SEATA_TC_WORK_DIR=self.work.as_posix(),LOCALAPPDATA='D:/Temp/seata-localappdata')
            args=[reg.JAVA,'-Xlog:class+load=info:file=class-load.log','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(self.work),'-Dseata.callback.fixture.resource='+resource,'-Dspring.config.location=file:'+(self.work/'application.yml').as_posix(),'-Dloader.path='+extension.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher','-h','127.0.0.1']
            tc=support.spawn(args,self.work,self.env,self.out,'tc');self.processes.append(tc);support.wait_owned(tc,38091,env)
            for role,port,mode in [('RM',38092,'basic'),('TM',38093,'basic')]:
                process=support.spawn([sys.executable,HERE/'java_proxy.py','--listen-port',str(port),'--upstream-port','38091','--mode',mode,'--role',role,'--events',self.out/(role+'-events.jsonl'),'--stop-file',self.stop_file],self.work,self.env,self.out,role+'-proxy');self.processes.append(process);support.wait_owned(process,port,env)
            support.save(self.out/'owned-processes.json',dict(tc_pid=tc.pid,proxy_pids=[p.pid for p in self.processes[1:]],tc_command=[str(v) for v in args],jar_sha256=reg.JAR_SHA,java_pin=reg.PIN,auth_spi='exact resource fixture delegates historical identity checker',new_java_fixture_compiled=True,production_auth_qualified=False))
            env.update(SEATA_RM_ENDPOINT='127.0.0.1:38092',SEATA_TM_ENDPOINT='127.0.0.1:38093')
        except BaseException:
            self.stop();raise
    def stop(self):
        if self.closed:return
        self.closed=True
        if not self.stop_file.exists():support.put(self.stop_file,'complete\n')
        exits=[]
        for p in reversed(self.processes):
            if p is not self.processes[0]:
                try:p.wait(timeout=7)
                except Exception:support.stop(p)
            else:support.stop(p)
            exits.append(dict(pid=p.pid,exit_code=p.returncode))
        if (self.work/'class-load.log').exists():support.put(self.out/'class-load.log',(self.work/'class-load.log').read_bytes())
        remaining={str(port):support.listeners(port,self.env) for port in [38091,38092,38093]}
        support.save(self.out/'cleanup.json',dict(processes=exits,listeners=remaining,runtime_retained=str(self.work)))
        need(not any(remaining.values()),'Owned Java listener remains')

def native_rows(rows):
    pending={};callbacks={};used=set()
    need(all(row['index']==i for i,row in enumerate(rows)),'Observer row sequence differs')
    need([row['monotonic_ns'] for row in rows]==sorted(row['monotonic_ns'] for row in rows),'Native observation ordering differs')
    for row in rows:
        if row['event']!='frame':continue
        raw=bytes.fromhex(row['frame_hex']);need(len(raw)>=16 and raw[:3]==b'\xda\xda\x01' and struct.unpack('>I',raw[3:7])[0]==len(raw) and raw[7:9]==b'\x00\x10' and raw[10:12]==b'\x01\x00','Original native envelope differs')
        typ=raw[9];wire=struct.unpack('>i',raw[12:16])[0];body=raw[16:];kind=struct.unpack('>H',body[:2])[0] if body else None
        need((typ,wire,kind)==(row['request_type'],row['request_id'],row['kind']) and row['connection']>0,'Native metadata not bound to raw bytes')
        if typ in [3,4]:need(not body,'Heartbeat body differs');continue
        need(typ in [0,1] and kind is not None,'Native request mode differs');key=(row['connection'],wire)
        if row['direction']=='c2s' and typ==0:
            need(key not in used and kind in [101,103,1,7,11,15],'Unexpected/reused original request');used.add(key);pending[key]=kind
        elif row['direction']=='s2c' and typ==1:
            need(key in pending and kind==pending.pop(key)+1,'Response lost original connection/ID/family')
        elif row['direction']=='s2c' and typ==0:
            need(kind==3 and key not in callbacks,'Unexpected callback');callbacks[key]=support.branch_identity(row,True)
        else:
            need(row['direction']=='c2s' and typ==1 and key in callbacks and kind==4 and body[2:4]==b'\x01\x00','Callback reply lost original connection/ID/family')
            original=callbacks.pop(key);xid,at=support.string(body,4);need(len(body)==at+9 and (xid,struct.unpack('>q',body[at:at+8])[0],body[-1])==(original['xid'],original['branch_id'],5),'Callback reply identity/status differs')
    need(not pending and not callbacks,'Unpaired original native request')


def validate(out,observation,events=None,journal=None):
    events=events or {role:[json.loads(line) for line in (out/(role+'-events.jsonl')).read_text('utf-8').splitlines()] for role in ['RM','TM']}
    for role,rows in events.items():
        need(rows[-1]['event']=='summary' and rows[-1]['succeeded'] is True and rows[-1]['mode']=='basic' and rows[-1]['faults']==0 and not any(r['event']=='error' for r in rows),'Original native observer failed')
        native_rows(rows)
    rm=[r for r in events['RM'] if r['event']=='frame'];tm=[r for r in events['TM'] if r['event']=='frame']
    need(all(r['action']=='forward' and r['delivery_count']==1 for r in rm+tm),'Unexpected native replay/fault')
    def one(rows,kind,direction):
        selected=[r for r in rows if r['kind']==kind and r['direction']==direction];need(len(selected)==1,'Unique original family required: '+str(kind));return selected[0]
    register=one(rm,11,'c2s');positive=one(rm,12,'s2c');callback=one(rm,3,'s2c');result=one(rm,4,'c2s')
    request=support.branch_identity(register,False);original=support.branch_identity(callback,True)
    need(request['xid']==original['xid']==observation['xid'] and request['resource']==original['resource']==observation['resource_id'] and request['lock']=='undo_work:1' and request['branch_type']==original['branch_type']==0 and request['data']==original['data']=='','Original AT registration/callback identity differs')
    data=support.payload(positive);need(len(data)==12 and data[:4]==b'\x00\x0c\x01\x00' and struct.unpack('>q',data[4:])[0]==original['branch_id']==observation['branch_id'],'Actual original branch ID differs')
    need(register['connection']==positive['connection']==callback['connection']==result['connection'] and register['request_id']==positive['request_id'] and callback['request_id']==result['request_id'],'Original connection/request ID differs')
    need(support.payload(result)[2:4]==b'\x01\x00' and support.payload(result)[-1]==5,'Original callback did not confirm commit cleanup')
    begin=one(tm,2,'s2c');end=one(tm,7,'c2s');end_result=one(tm,8,'s2c')
    need(support.string(support.payload(begin),4)[0]==support.string(support.payload(end),2)[0]==observation['xid'],'Original TM XID differs')
    need(support.payload(end_result)[2:]==b'\x01\x00\x09','Original TC global commit response differs')
    statuses=[r for r in tm if r['kind']==16 and r['direction']=='s2c'];requests=[r for r in tm if r['kind']==15 and r['direction']=='c2s']
    need(statuses and len(statuses)==len(requests) and all(support.string(support.payload(r),2)[0]==observation['xid'] for r in requests),'Original status request identity differs')
    need(all(support.payload(r)[2:4]==b'\x01\x00' and support.payload(r)[4] in [8,9,15] and len(support.payload(r))==5 for r in statuses) and support.payload(statuses[-1])[-1]==15,'Original async commit cleanup status differs')
    need(positive['monotonic_ns']<end['monotonic_ns']<callback['monotonic_ns']<result['monotonic_ns']<statuses[-1]['monotonic_ns'] and end['monotonic_ns']<end_result['monotonic_ns'],'Original asynchronous commit order differs')
    spec=importlib.util.spec_from_file_location('original_commit_journal_reader',ROOT/'tools/formal-rm-java/validate.py');reader=importlib.util.module_from_spec(spec);spec.loader.exec_module(reader)
    journal=journal if journal is not None else (out.parent/'observations/java-branches.log').read_bytes();records=reader.journal_records(journal,support.payload(callback))
    auth=json.loads((out/'auth-binding.json').read_bytes());need(auth['resource']==observation['resource_id'] and auth['compiled_sha256']==support.sha(out/'CallbackOwnershipAuthHandler.class'),'Exact auth resource/artifact differs')
    loaded={m[1]:m[2].replace('\\','/') for m in re.finditer(r'\]\s+(\S+)\s+source:\s+([^\r\n]+)',(out/'class-load.log').read_text('utf-8'))};origins={}
    for name in ['org.apache.seata.server.auth.CallbackOwnershipAuthHandler','org.apache.seata.server.auth.Phase0CredentialCheckAuthHandler']:
        need('/fixture/' in loaded.get(name,''),'Auth fixture origin differs');origins[name]=loaded[name]
    for name in ['org.apache.seata.server.coordinator.DefaultCore','org.apache.seata.server.coordinator.AbstractCore','org.apache.seata.core.rpc.processor.server.RegRmProcessor','org.apache.seata.core.rpc.processor.server.RegTmProcessor','org.apache.seata.server.AbstractTCInboundHandler']:
        need((reg.ORACLE/'server/target/seata-server.jar').as_posix() in loaded.get(name,''),'Original Java class origin differs');origins[name]=loaded[name]

    need(observation['global_result']=='Committed' and observation['file_result']=='Completed' and observation['schema_version']==3 and observation['origin']==1 and observation['phase']==1 and observation['rollback_terminal']==0 and observation['commit_authorized']==observation['commit_terminal']==1 and observation['lock_key']=='undo_work:1' and observation['business_rows']==1 and observation['business']=='committed_java_fixture' and len(observation['remaining_undo'])==1 and observation['remaining_undo'][0]!=observation['branch_id'] and observation['registration_result']=='Success' and observation['local_rollback_before_fixture_commit'] is True and observation['fixture_committed_via_admin'] is True and observation['commit_preconditions_locked'] is True and observation['uncommitted_Normal_observed'] is True and observation['business_replayed'] is False and observation['public_business_commit'] is False,'Independent original commit fixture facts differ')
    return dict(verified=True,actual_Java_TC=True,java_pin=reg.PIN,jar_sha256=reg.JAR_SHA,real_branch_id=observation['branch_id'],original_registration_requests=1,original_commit_callbacks=1,journal_records=records,class_origins=origins,fixture_committed_via_admin=True,public_business_commit=False,TC_full_safety_gate_passed=False)

def negative_controls(out,observation):
    events={role:[json.loads(line) for line in (out/(role+'-events.jsonl')).read_text('utf-8').splitlines()] for role in ['RM','TM']};cases=[]
    def reject(name,changed,obs,journal=None):
        try:validate(out,obs,changed,journal)
        except (ValueError,KeyError,IndexError):cases.append(name);return
        raise ValueError('Original commit control accepted: '+name)
    for role,kind in [('RM',11),('RM',12),('RM',3),('RM',4),('TM',7),('TM',8)]:
        changed=copy.deepcopy(events);changed[role]=[r for r in changed[role] if r.get('kind')!=kind];reject('missing-original-'+str(kind),changed,observation)
    for key,value in [('branch_id',observation['branch_id']^1),('origin',2),('lock_key',None),('commit_authorized',0),('commit_terminal',0),('business_rows',0),('registration_result','Unknown'),('fixture_committed_via_admin',False),('commit_preconditions_locked',False)]:
        changed=copy.deepcopy(observation);changed[key]=value;reject('wrong-'+key,events,changed)
    for kind in [11,3]:
        changed=copy.deepcopy(events);row=next(r for r in changed['RM'] if r.get('kind')==kind);raw=bytearray.fromhex(row['frame_hex']);length=struct.unpack('>h',raw[18:20])[0];raw[20+length+(8 if kind==3 else 0)]=1;row['frame_hex']=raw.hex();reject('wrong-AT-mode-'+str(kind),changed,observation)
    journal=bytearray((out.parent/'observations/java-branches.log').read_bytes());journal[-1]^=1;reject('journal-checksum',events,observation,bytes(journal))
    return dict(all_rejected=True,count=len(cases),cases=cases)
