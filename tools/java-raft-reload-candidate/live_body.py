# Owned lifecycle adapted from the sealed original runner; candidate gates are explicit.
import datetime,json,socket,subprocess,time,urllib.request,urllib.error,urllib.parse
from pathlib import Path
import checks
import native_observer
from finalization import checked_success
NATIVE=[28091,28093,28095];RAFT=[28092,28094,28096];CONTROL=[28101,28102,28103]
OPENER=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def execute(args,h,output,runtime,jar,classes,compiled,reader_classes,probe,token,source_inputs,dependencies,inventory):
    from runtime import ROOT,MAIN,verify_final_inputs
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
                command=[h.JAVA,'-Xlog:class+load=info','-XX:ActiveProcessorCount=2','-Xmx512m','-Djava.io.tmpdir=D:/Temp','-Duser.home='+node.as_posix(),
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
                        leader=leaders[0];checks.verify_completion(values[leader]);committed=values[leader]['committed']
                        if any(v['applied']<committed or v['leader_id']!=values[leader]['leader_id'] for v in values.values()):raise RuntimeError('Replicas have not caught up')
                        if verify:
                            for value in values.values():checks.verify_records(value,status,args.host,after_election)
                        repeated=api(leader);checks.verify_completion(repeated)
                        if any(repeated[key]!=values[leader][key] for key in ['term','node_id','leader_id','reload_complete_term']):raise RuntimeError('Leader changed across repeated sample')
                        h.write(directory/f'{stage}-leader-confirmation.json',json.dumps(repeated,indent=2)+'\n')
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
                for record_index,wanted in enumerate(expected):
                    events=[]
                    try:
                        with socket.create_connection(('127.0.0.1',NATIVE[index]),timeout=5) as stream:
                            observed=native_observer.query(stream,f'{args.host}:28091:{940000+record_index}',f'mysql://raft-retry-{record_index}','orders:1',wanted,status==16 or record_index!=0,events)
                        h.write(directory/f'{stage}-native-{record_index}.json',json.dumps(observed,indent=2)+'\n')
                    finally:
                        h.write(directory/f'{stage}-native-{record_index}-frames.json',json.dumps(events,indent=2)+'\n')
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
                events=read_events(0);found=checks.reload_exceptions(events,status,boundary);attempt=0
                if found:raise RuntimeError('Candidate actual reload still throws in transition window')
                checks.verify_completion(api(index))
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
                original_mutation_replays=0,actual_snapshot_reload=True,diagnostic_only=False))
            for process in processes.values():h.stop(process)
            for log in directory.glob('node_*_generation_*/tc-stdout.log'):
                text=log.read_text(encoding='utf-8',errors='replace')
                for name in ['org.apache.seata.server.cluster.raft.RaftStateMachine','org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.cluster.raft.RaftReloadObservationControl']:
                    line=next((line for line in text.splitlines() if (' '+name+' source: ') in line),'')
                    if not line or classes.as_uri().replace('file:///','file:/') not in line:raise RuntimeError('Actual runtime class source differs: '+name)
            processes.clear()
            print(f'PASS live RAFT candidate status={status} originalRecords=2 transitions=2 snapshotReload=true',flush=True)
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
        success=checked_success(success,errors,lambda:verify_final_inputs(output,source_inputs,dependencies,inventory,classes))
        if h.sha(jar)!=h.JAR_HASH:errors.append('Original JAR changed during run')
        result=dict(executed_at=datetime.datetime.now().astimezone().isoformat(),succeeded=success and not errors,java_pin=h.PIN,jar_sha256=h.sha(jar),helper_sha256=h.sha(compiled),native_observer_sha256=h.sha(Path(native_observer.__file__)),jfr_reader_sha256=h.sha(reader_classes/'ReadRaftExceptions.class'),
            cohorts=cohorts,owned_pids=all_pids,cleanup_errors=errors,ports_released=not listeners,
            scope='Independent preface+retention+completion observation candidate, real 3-node elections/snapshots, seeded records only; no natural deadlines, operator retry, real RM compensation or failed-leader fencing claim')
        h.write(output/'result.json',json.dumps(result,indent=2)+'\n')
        print('EVIDENCE '+str(output),flush=True)
    if not result['succeeded']:raise RuntimeError('Live RAFT diagnostics incomplete')
