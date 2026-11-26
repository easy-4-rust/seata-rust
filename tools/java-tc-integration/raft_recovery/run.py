"""Owned three-node original TC/JRaft elections, snapshots and retry-timeout reload diagnostics."""
import argparse
import datetime
import importlib.util
import json
import re
import socket
import subprocess
import time
import urllib.request
import urllib.error
import urllib.parse
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

import checks

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('seata_raft_support',ROOT/'tools/java-tc-integration/backend_recovery/run.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
TYPE='RaftRetryTimeoutControl'
MAIN='org.apache.seata.server.cluster.raft.'+TYPE
NATIVE=[28091,28093,28095]
RAFT=[28092,28094,28096]
CONTROL=[28101,28102,28103]
OPENER=urllib.request.build_opener(urllib.request.ProxyHandler({}))


def run(args):
    build=Path(args.java_build);oracle=Path(args.java_source);base=Path(args.runtime_base)
    if any(not re.match(r'^[DE]:[/\\]',str(p)) for p in [base,build]):raise ValueError('All development paths must remain on D/E')
    if not re.fullmatch(r'(?:\d{1,3}\.){3}\d{1,3}',args.host):raise ValueError('Explicit local IPv4 required')
    # Bind the local peer address before starting anything; no remote cluster may be targeted.
    for port in NATIVE+RAFT+CONTROL:
        with socket.socket() as listener:listener.bind((args.host if port in RAFT else '0.0.0.0',port))
    if h.command(['git','-C',oracle,'rev-parse','HEAD']).stdout.decode().strip()!=h.PIN:raise RuntimeError('Java pin mismatch')
    jar=oracle/'server/target/seata-server.jar'
    if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original TC JAR mismatch')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    token='phase0-raft-'+stamp.rsplit('_',1)[1]
    runtime=base/stamp;runtime.mkdir(parents=True,exist_ok=False)
    output=ROOT/'docs/phase-0/evidence/local-raft-retry-timeout'/('run_'+stamp);output.mkdir(parents=True,exist_ok=False)
    reader_classes=runtime/'jfr-reader-classes';reader_classes.mkdir()
    h.logged([h.JAVA.parent/'javac.exe','-encoding','UTF-8','-d',reader_classes,ROOT/'tools/java-tc-integration/raft_recovery/ReadRaftExceptions.java'],output,'jfr-reader-build.log')
    test=build/'test-suite/test-new-version'
    before=h.audit_build(oracle,build)
    for name in [TYPE,TYPE+'Test']:
        (test/f'src/test/java/org/apache/seata/server/{name}.java').write_bytes((ROOT/f'tools/java-v1-golden/{name}.java').read_bytes())
        leaf=test/f'target/test-classes/org/apache/seata/server/cluster/raft/{name}.class'
        if leaf.exists():leaf.unlink()
    with (output/'java-tests.log').open('xb') as log:
        result=subprocess.run([str(h.MAVEN),'-B','-ntp','-s',str(h.SETTINGS),'-f',str(build/'pom.xml'),'-pl','test-suite/test-new-version','-am',
            '-Dtest='+TYPE+'Test','-Dsurefire.failIfNoSpecifiedTests=false','-Djava.io.tmpdir=D:/Temp','test'],
            env=h.ENV,stdout=log,stderr=subprocess.STDOUT,creationflags=h.FLAGS,timeout=300)
    if result.returncode:raise RuntimeError('Fresh RAFT control tests failed')
    after=h.audit_build(oracle,build)
    h.write(output/'java-production-source-audit.json',json.dumps(dict(java_pin=h.PIN,before_build=before,after_build=after,normalization='CRLF to LF only'),indent=2)+'\n')
    report=test/f'target/surefire-reports/TEST-{MAIN}Test.xml';suite=ET.parse(report).getroot()
    if suite.get('tests')!='4' or any(int(suite.get(k,'0')) for k in ['failures','errors','skipped']):raise RuntimeError('Fresh Java guard report mismatch')
    h.write(output/'java-report.xml',report.read_bytes())
    for name in [TYPE,TYPE+'Test']:
        if (test/f'src/test/java/org/apache/seata/server/{name}.java').read_bytes()!=(ROOT/f'tools/java-v1-golden/{name}.java').read_bytes():raise RuntimeError('Reconcile formatter output before live run')
    classes=runtime/'control-classes';compiled=classes/'org/apache/seata/server/cluster/raft'/f'{TYPE}.class';compiled.parent.mkdir(parents=True)
    h.write(compiled,(test/f'target/test-classes/org/apache/seata/server/cluster/raft/{TYPE}.class').read_bytes())
    h.logged(['cargo','build','--manifest-path',ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'],output,'rust-build.log')
    probe=Path(h.ENV['CARGO_TARGET_DIR'])/'debug/seata-java-tc-it.exe'
    processes={};cohorts=[];success=False;errors=[];all_pids=[]
    try:
        for status in [1,16,17]:
            work=runtime/f'status_{status}';work.mkdir();directory=output/f'status_{status}';directory.mkdir()
            peers=','.join(f'{args.host}:{p}' for p in RAFT)
            generations={i:0 for i in range(3)}
            def start(index):
                node=work/f'node_{index}';node.mkdir(exist_ok=True)
                if not (node/'.phase0-owned-raft').exists():h.write(node/'.phase0-owned-raft',token+'\n')
                config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text().replace('18091',str(NATIVE[index]))
                prefix,store=config.split('  store:\n',1)
                store=store.replace('mode: file','mode: raft')
                prefix=prefix.replace('  server:\n','  server:\n    raft:\n      group: default\n      server-addr: '+peers+'\n      sync: true\n      snapshot-interval: 600\n      election-timeout-ms: 1000\n      reporter-enabled: false\n      serialization: jackson\n      compressor: none\n')
                config=prefix+'  store:\n'+store
                if not (node/'application.yml').exists():h.write(node/'application.yml',config)
                generation=generations[index];generations[index]+=1
                logs=directory/f'node_{index}_generation_{generation}';logs.mkdir()
                h.write(logs/'application.yml',config)
                env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=node.as_posix()
                command=[h.JAVA,'-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+node.as_posix(),
                    '-Dserver.raftPort='+str(RAFT[index]),'-Dspring.config.location=file:'+(node/'application.yml').as_posix(),
                    '-Dseata.it.raft.dir='+node.as_posix(),'-Dseata.it.raft.controlPort='+str(CONTROL[index]),
                    '-Dseata.it.raft.token='+token,'-Dseata.it.raft.host='+args.host,'-Dloader.main='+MAIN,
                    '-Dloader.path='+classes.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher',
                    '-h',args.host,'-n',str(index+1)]
                process=h.spawn(command,logs,'tc',node,env);processes[index]=process;all_pids.append(process.pid)
                h.write(logs/'process.json',json.dumps(dict(pid=process.pid,index=index,generation=generation,command=[str(c) for c in command]),indent=2)+'\n')
            def api(index,path='/state',method='GET',archive=None):
                request=urllib.request.Request(f'http://127.0.0.1:{CONTROL[index]}'+path,method=method,headers={'X-Phase0-Owner':token})
                try:
                    with OPENER.open(request,timeout=20) as response:body=json.load(response)
                except urllib.error.HTTPError as error:
                    raise RuntimeError(f'Owned node{index} {path} HTTP{error.code}: {error.read().decode("utf-8")}') from error
                if body.get('pid')!=processes[index].pid:raise RuntimeError('Control response belongs to another process')
                if archive:h.write(directory/archive,json.dumps(body,indent=2)+'\n')
                return body
            def stable(stage,live,after_election=False,verify=True):
                deadline=time.monotonic()+45
                while True:
                    for i in live:
                        if processes[i].poll() is not None:raise RuntimeError(f'Owned node {i} exited: {processes[i].returncode}')
                    try:
                        values={i:api(i) for i in live}
                        leaders=[i for i,v in values.items() if v['leader']]
                        if len(leaders)!=1:raise RuntimeError('No unique leader')
                        leader=leaders[0];committed=values[leader]['committed']
                        if any(v['applied']<committed or v['leader_id']!=values[leader]['leader_id'] for v in values.values()):raise RuntimeError('Replicas have not caught up')
                        if verify:
                            for value in values.values():checks.verify_records(value,status,args.host,after_election)
                        for i,value in values.items():h.write(directory/f'{stage}-node_{i}.json',json.dumps(value,indent=2)+'\n')
                        return leader,values
                    except (OSError,RuntimeError,ValueError) as error:
                        if time.monotonic()>=deadline:raise RuntimeError(f'{stage} did not settle: {error}') from error
                        time.sleep(.2)
            def visibility(stage,index):
                expected=[5,15] if status==1 else [status,status]
                for record_index,wanted in enumerate(expected):
                    xid=f'{args.host}:28091:{940000+record_index}'
                    params=urllib.parse.urlencode(dict(xid=xid,pageNum=1,pageSize=20,withBranch='true',unit='default'))
                    request=urllib.request.Request(f'http://127.0.0.1:{NATIVE[index]}/api/v1/console/globalSession/query?'+params)
                    with OPENER.open(request,timeout=10) as response:body=json.load(response)
                    h.write(directory/f'{stage}-console-{record_index}.json',json.dumps(body,indent=2)+'\n')
                    absent=wanted==15
                    if str(body.get('code'))!='200' or body.get('total')!=(0 if absent else 1) or len(body.get('data',[]))!=(0 if absent else 1):raise RuntimeError('Actual TC console visibility differs')
                    if not absent:
                        row=body['data'][0];branches=row.get('branchSessionVOs') or []
                        if row.get('xid')!=xid or row.get('status')!=wanted or len(branches)!=(1 if record_index==0 else 0):raise RuntimeError('Actual TC console original record differs')
                        if branches and (int(branches[0]['branchId'])!=950000 or branches[0].get('resourceId')!='mysql://raft-retry-0'):raise RuntimeError('Actual console branch identity/resource differs')
                env=h.ENV.copy();env.update(SEATA_TC_HOST='127.0.0.1',SEATA_TC_PORT=str(NATIVE[index]),SEATA_IT_APP_ID='raft-read-only',
                    SEATA_IT_RAFT_XIDS=','.join(f'{args.host}:28091:{940000+i}' for i in range(2)),SEATA_IT_RAFT_STATUSES=','.join(map(str,expected)),SEATA_IT_HOLD_MS='0')
                result=subprocess.run([str(probe),'--query-raft-timeout'],cwd=work,env=env,capture_output=True,timeout=20,creationflags=h.FLAGS)
                h.write(directory/f'{stage}-native.txt',result.stdout+result.stderr)
                if result.returncode or result.stdout.count(b'RAFT_STATUS xid=')!=2:raise RuntimeError('Independent native V1 original-state queries failed')
            for index in range(3):start(index)
            leader,initial=stable('initial',[0,1,2],verify=False)
            api(leader,'/seed?'+str(status),'POST','seed.json')
            leader,seeded=stable('seeded',[0,1,2])
            for index in range(3):api(index,'/snapshot','POST',f'snapshot-node_{index}.json')
            leader,snapshots=stable('snapshotted',[0,1,2])
            boundary_one=datetime.datetime.now(datetime.timezone.utc).isoformat()
            old=leader;h.stop(processes[old]);live=[i for i in range(3) if i!=old]
            leader,first=stable('election_one',live,after_election=True)
            checks.verify_transition(snapshots[old],first[leader],snapshots[old]['node_id'])
            visibility('election_one',leader)
            def inspect_jfr(index,label,boundary):
                def read_events(attempt):
                    candidate=label+'_'+chr(ord('a')+attempt)
                    api(index,'/dump?'+candidate,'POST','dump-'+candidate+'.json')
                    source=work/f'node_{index}'/(candidate+'.jfr')
                    raw=h.command([h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Dfile.encoding=UTF-8','-cp',reader_classes,'ReadRaftExceptions',source],timeout=10).stdout
                    h.write(directory/(candidate+'-events.json'),raw);h.write(directory/(candidate+'.jfr'),source.read_bytes())
                    return json.loads(raw)['recording']['events']
                if status in [16,17]:found,attempt=checks.wait_for_reload(read_events,status,boundary,max_attempts=15,delay=.2)
                else:
                    events=read_events(0);found=[];attempt=0
                    if checks.reload_exceptions(events,1,boundary):raise RuntimeError('Positive Begin control unexpectedly throws')
                h.write(directory/(label+'-reload-exceptions.json'),json.dumps(dict(boundary=boundary,dump_attempt=attempt,events=found),indent=2)+'\n')
                return len(found)
            first_errors=inspect_jfr(leader,'election_one',boundary_one)
            start(old)
            leader,restarted=stable('restarted',[0,1,2],after_election=True)
            if restarted[old]['pid']==initial[old]['pid']:raise RuntimeError('Old node was not a fresh JVM')
            restart_log=directory/f'node_{old}_generation_1/tc-stdout.log'
            if b'on snapshot load end index:' not in restart_log.read_bytes():raise RuntimeError('Restart did not demonstrate actual session snapshot loading')
            visibility('restarted',leader)
            boundary_two=datetime.datetime.now(datetime.timezone.utc).isoformat()
            second_old=leader;h.stop(processes[second_old]);live=[i for i in range(3) if i!=second_old]
            leader,second=stable('election_two',live,after_election=True)
            checks.verify_transition(restarted[second_old],second[leader],restarted[second_old]['node_id'])
            visibility('election_two',leader)
            second_errors=inspect_jfr(leader,'election_two',boundary_two)
            cohorts.append(dict(seeded_status=status,original_records=2,original_branches=1,first_leader=old,new_leader=[i for i,v in first.items() if v['leader']][0],second_leader=leader,
                original_pid=initial[old]['pid'],restarted_pid=restarted[old]['pid'],first_reload_exceptions=first_errors,second_reload_exceptions=second_errors,
                original_mutation_replays=0,actual_snapshot_reload=True,diagnostic_only=status!=1))
            for process in processes.values():h.stop(process)
            processes.clear()
            print(f'PASS live RAFT diagnostic status={status} originalRecords=2 transitions=2 snapshotReload=true',flush=True)
        success=True
    finally:
        for process in processes.values():
            try:h.stop(process)
            except Exception as error:errors.append(str(error))
        def port_open(port):
            try:
                with socket.create_connection((args.host if port in RAFT else '127.0.0.1',port),timeout=.2):return True
            except OSError:return False
        listeners=[port for port in NATIVE+RAFT+CONTROL if port_open(port)]
        if listeners:errors.append('Owned test ports remain open: '+str(listeners))
        result=dict(executed_at=datetime.datetime.now().astimezone().isoformat(),succeeded=success and not errors,java_pin=h.PIN,jar_sha256=h.sha(jar),helper_sha256=h.sha(compiled),rust_sha256=h.sha(probe),jfr_reader_sha256=h.sha(reader_classes/'ReadRaftExceptions.class'),
            cohorts=cohorts,owned_pids=all_pids,cleanup_errors=errors,ports_released=not listeners,
            scope='Real 3-node original TC/JRaft replication/snapshot/leader transitions; injected timeout records, no natural deadlines, operator retry or real RM compensation; reload exception reproduction is diagnostic, not recovery acceptance')
        h.write(output/'result.json',json.dumps(result,indent=2)+'\n')
        (output.parent/'latest_run.json').write_text(json.dumps(dict(evidence_directory=str(output),runtime_directory=str(runtime),succeeded=result['succeeded']),indent=2)+'\n',encoding='utf-8')
    if not result['succeeded']:raise RuntimeError('Live RAFT diagnostics incomplete')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--java-source',default='E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle');parser.add_argument('--java-build',default='D:/codex-builds/seata-java-exact-overlay-20261112');parser.add_argument('--runtime-base',default='D:/codex-builds/seata raft retry timeout');parser.add_argument('--host',default='192.168.1.10');run(parser.parse_args())
