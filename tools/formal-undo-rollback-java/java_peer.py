"""Compile original MySQL/abstract Undo managers and compare actual JDBC store/read/delete against native rows."""
import datetime
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid
import zipfile

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;JAVA=ROOT.parent/'seata-java-phase09-oracle'
BUILD=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34';LOCK=ROOT/'docs/phase-2/evidence/lock-key-peer/run_20261116_004741_eaf663fb'
sys.path.insert(0,str(ROOT/'tools/formal-undo-format'));from runtime_binding import RuntimeBinding
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_bytes())
def need(v,m):
    if not v:raise ValueError(m)
def validate_rows(rows,xid):
    need(all(r['xid']==xid for r in rows),'Archived original child/root XID differs')
    need(all(type(r['status']) is int and r['status']==0 for r in rows),'Archived original child/root Normal state differs')
    need(all(r['log_created']==r['log_modified'] and type(r['log_created']) is str for r in rows),'Archived original insertion timestamps differ')
def run(parent,work,env):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/undo-rollback-java-peer'/('run_'+stamp);out.mkdir(parents=True)
    runtime=Path('D:/codex-builds')/('seata_undo_rollback_java_'+stamp);(runtime/'classes').mkdir(parents=True)
    print('UNDO_ROLLBACK_JAVA_PEER_RUN '+str(out),flush=True)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW);put(name,v.stdout);need(v.returncode==0,name+' failed '+v.stdout[-1600:].decode(errors='replace'));return v.stdout
    cases=[];seen=set()
    for path in sorted((parent/'observations').glob('native-case-*.json')):
        value=read(path);key=json.dumps({k:value[k] for k in ['initial','statements','profile','packet']},sort_keys=True)
        if key in seen:continue
        seen.add(key);need(all(row['xid']=='rollback:xid' and row['status']==0 and row['created']==row['modified'] for row in value['rows']),'Actual native row identity/state/timestamp differs')
        value['case']=path.name;cases.append(value)
    need(len(cases)==10 and {case['profile'] for case in cases}=={'default','numeric_tinyint'},'Complete actual native source case/profile set differs')
    save('cases.json',cases);(runtime/'cases.json').write_bytes((out/'cases.json').read_bytes())
    for packet in [BUILD,LOCK]:
        for row in read(packet/'catalog.json')['members']:need(sha(packet/row['path'])==row['sha256'],'Inherited Java authority drift')
    binding=RuntimeBinding(BUILD,JAVA);lock=read(LOCK/'runtime.json')
    for row in read(LOCK/'runtime-inputs.json'):
        path=Path(row['path']).resolve()
        if path not in binding.files:
            need(sha(Path(row['archive']))==row['sha256'],'Original lock classpath archive differs');binding.files[path]={key:row[key] for key in ['sha256','archive','role']}
    binding.roots.append((Path(lock['runtime'])/'classes').resolve());binding.verify()
    server=JAVA/'server/target/seata-server.jar';need(sha(server)=='aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5','Original TC jar differs')
    with zipfile.ZipFile(server) as archive:extra=archive.read('BOOT-INF/lib/fastjson-1.2.83.jar')
    library=runtime/'fastjson-1.2.83.jar';library.write_bytes(extra);put('libraries/fastjson-1.2.83.jar',extra);binding.files[library.resolve()]=dict(sha256=sha(library),archive=str(out/'libraries/fastjson-1.2.83.jar'),role='original_TC_provided_fastjson_compile_dependency')
    classpath=lock['classpath']+os.pathsep+str(library);binding.verify()
    sources=[JAVA/('rm-datasource/src/main/java/org/apache/seata/rm/datasource/'+leaf) for leaf in ['DataCompareUtils.java','undo/AbstractUndoExecutor.java','undo/AbstractUndoLogManager.java','undo/mysql/MySQLUndoLogManager.java','undo/mysql/MySQLUndoInsertExecutor.java','undo/mysql/MySQLUndoUpdateExecutor.java','undo/mysql/MySQLUndoDeleteExecutor.java']]
    need(command(['git','-C',JAVA,'rev-parse','HEAD'],'java-pin.txt').decode().strip()=='cba73e180fd8896a99df6415b859d275122feaa7','Java pin differs')
    for i,path in enumerate(sources):
        blob=command(['git','-C',JAVA,'show','cba73e180fd8896a99df6415b859d275122feaa7:'+path.relative_to(JAVA).as_posix()],f'original-{i}.java')
        need(path.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Selected original source differs')
    paths={Path(__file__),HERE/'UndoRollbackOracle.java',ROOT/'tools/formal-undo-format/runtime_binding.py',ROOT/'docs/phase-2/mysql-undo-rollback-contract.md',server,*sources}
    paths.update(p for p in (parent/'observations').rglob('*') if p.is_file());frozen=[]
    for path in sorted(paths):
        name='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(name,path.read_bytes());frozen.append(dict(path=str(path),snapshot=name,sha256=sha(path)))
    save('source-inputs.json',frozen)
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-cp',classpath,'-d',runtime/'classes',*sources,HERE/'UndoRollbackOracle.java'],'compile.log')
    for path in (runtime/'classes').rglob('*.class'):
        name='classes/'+path.relative_to(runtime/'classes').as_posix();put(name,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/name),role='compiled_original_Undo_manager')
    binding.roots.append((runtime/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest())
    driver=binding.drivers['8.0.27'];cp=str(runtime/'classes')+os.pathsep+classpath+os.pathsep+str(driver)
    loaded=runtime/'class-load.log';url=f"jdbc:mysql://127.0.0.1:{env['SEATA_MYSQL_PORT']}/{env['SEATA_MYSQL_DATABASE']}?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC&connectTimeout=3000&socketTimeout=10000&allowMultiQueries=true"
    java_args=['D:/Java/jdk-17.0.12/bin/java.exe','-Duser.timezone=UTC','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dlogback.configurationFile='+str(Path(read(BUILD/'build.json')['runtime'])/'console.xml'),'-Xlog:class+load=info:file='+loaded.as_posix(),'-cp',cp,'UndoRollbackOracle',url,'root',env['SEATA_MYSQL_PASSWORD'],runtime/'cases.json',runtime/'observations.json',work/'init/00-undo.sql']
    command(java_args,'run.log')
    controls=[]
    for probe,rejection in [('skip_cleanup','Original Undo root/children cleanup mismatch'),('drop_retained','Original dirty Undo bytes changed')]:
        args=list(map(str,java_args));args=[v for v in args if not v.startswith('-Xlog:')]
        value=subprocess.run(args+[probe],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW);put('controls/'+probe+'.log',value.stdout)
        need(value.returncode==1 and rejection.encode() in value.stdout,'Actual Java cleanup/retention control failed to reject');controls.append(dict(probe=probe,actual_exit_code=value.returncode,actual_rejection=rejection))
    save('cleanup-negative-controls.json',controls)
    put('class-load.log',loaded.read_bytes());observed=read(runtime/'observations.json');need(len(observed)==24 and all(r['status']=='MATCH' and r['other_branch_unchanged'] is True and (r['dirty_undo_exactly_retained'] is True if r['dirty_rejected'] else r['root_children_deleted'] is True) for r in observed),'Original source manager comparisons differ');put('observations.json',(runtime/'observations.json').read_bytes())
    origins=binding.loaded(loaded,'8.0.27',True,'undo_rollback')
    for name in ['org.apache.seata.rm.datasource.'+p.relative_to(JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource').as_posix()[:-5].replace('/','.') for p in sources]:
        need(Path(origins['bound_origins'][name]['actual_file'])==runtime/'classes'/(name.replace('.','/')+'.class'),'New original source shadowed')
    save('origin-binding.json',origins);binding.verify()
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Java store peer input drift')
    save('result.json',dict(status='SELECTED_ORIGINAL_UNDO_EXECUTOR_METHODS_MATCH',comparisons=24,native_input_cases=10,actual_cleanup_negative_controls=2,original_newly_compiled_sources=7,actual_JDBC='8.0.27',actual_schema_and_database=env['SEATA_MYSQL_DATABASE'],
        archived_Rust_Normal_rows_reinserted_as_fixture=True,original_full_undo_invoked=False,original_JUnit_executed=False,SQL_Undo_commit_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));return dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))
