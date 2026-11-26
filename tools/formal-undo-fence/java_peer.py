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
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-2/evidence/undo-fence-java-peer'/('run_'+stamp);out.mkdir(parents=True)
    runtime=Path('D:/codex-builds')/('seata_undo_fence_java_'+stamp);(runtime/'classes').mkdir(parents=True)
    print('UNDO_FENCE_JAVA_PEER_RUN '+str(out),flush=True)
    def put(name,data):
        p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data.encode('utf-8') if isinstance(data,str) else data)
    def save(name,data):put(name,json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    def command(args,name):
        v=subprocess.run(list(map(str,args)),cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW);put(name,v.stdout);need(v.returncode==0,name+' failed '+v.stdout[-1600:].decode(errors='replace'));return v.stdout
    row=read(parent/'observations/global-finished.json');payload=read(parent/'observations/late-normal-payload.json')['bytes']
    need(row['xid']=='fence:xid' and row['branch_id']==42 and row['status']==1 and bytes(row['bytes'])==b'{}' and row['log_created']==row['log_modified'],'Original archived guard differs')
    save('cases.json',dict(row=row,normal_bytes=payload));(runtime/'cases.json').write_bytes((out/'cases.json').read_bytes())
    for packet in [BUILD,LOCK]:
        for row in read(packet/'catalog.json')['members']:need(sha(packet/row['path'])==row['sha256'],'Inherited Java authority drift')
    binding=RuntimeBinding(BUILD,JAVA);lock=read(LOCK/'runtime.json')
    for row in read(LOCK/'runtime-inputs.json'):
        path=Path(row['path']).resolve()
        if path not in binding.files:
            need(sha(Path(row['archive']))==row['sha256'],'Original lock classpath archive differs');binding.files[path]={key:row[key] for key in ['sha256','archive','role']}
    binding.roots.append((Path(lock['runtime'])/'classes').resolve());binding.verify()
    sources=[JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource/undo/AbstractUndoLogManager.java',JAVA/'rm-datasource/src/main/java/org/apache/seata/rm/datasource/undo/mysql/MySQLUndoLogManager.java']
    need(command(['git','-C',JAVA,'rev-parse','HEAD'],'java-pin.txt').decode().strip()=='cba73e180fd8896a99df6415b859d275122feaa7','Java pin differs')
    for i,path in enumerate(sources):
        blob=command(['git','-C',JAVA,'show','cba73e180fd8896a99df6415b859d275122feaa7:'+path.relative_to(JAVA).as_posix()],f'original-{i}.java')
        need(path.read_bytes().replace(b'\r\n',b'\n')==blob.replace(b'\r\n',b'\n'),'Selected original source differs')
    paths={Path(__file__),HERE/'UndoFenceOracle.java',ROOT/'tools/formal-undo-format/runtime_binding.py',ROOT/'docs/phase-2/mysql-undo-fence-contract.md',*sources}
    paths.update(p for p in (parent/'observations').rglob('*') if p.is_file());frozen=[]
    for path in sorted(paths):
        name='sources/'+hashlib.sha256(str(path).encode()).hexdigest()[:24]+'.bin';put(name,path.read_bytes());frozen.append(dict(path=str(path),snapshot=name,sha256=sha(path)))
    save('source-inputs.json',frozen)
    command(['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-cp',lock['classpath'],'-d',runtime/'classes',*sources,HERE/'UndoFenceOracle.java'],'compile.log')
    for path in (runtime/'classes').rglob('*.class'):
        name='classes/'+path.relative_to(runtime/'classes').as_posix();put(name,path.read_bytes());binding.files[path.resolve()]=dict(sha256=sha(path),archive=str(out/name),role='compiled_original_Undo_manager')
    binding.roots.append((runtime/'classes').resolve());binding.verify();save('runtime-inputs.json',binding.manifest())
    driver=binding.drivers['8.0.27'];cp=str(runtime/'classes')+os.pathsep+lock['classpath']+os.pathsep+str(driver)
    loaded=runtime/'class-load.log';url=f"jdbc:mysql://127.0.0.1:{env['SEATA_MYSQL_PORT']}/{env['SEATA_MYSQL_DATABASE']}?useSSL=false&allowPublicKeyRetrieval=true&serverTimezone=UTC&connectTimeout=3000&socketTimeout=5000"
    command(['D:/Java/jdk-17.0.12/bin/java.exe','-Duser.timezone=UTC','-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(runtime),'-Dlogback.configurationFile='+str(Path(read(BUILD/'build.json')['runtime'])/'console.xml'),
        '-Xlog:class+load=info:file='+loaded.as_posix(),'-cp',cp,'UndoFenceOracle',url,'root',env['SEATA_MYSQL_PASSWORD'],runtime/'cases.json',runtime/'observations.json'],'run.log')
    put('class-load.log',loaded.read_bytes());observed=read(runtime/'observations.json');need(len(observed)==2 and all(r['status_result']=='MATCH' and r['normal_error']==1062 for r in observed),'Original source manager comparisons differ');put('observations.json',(runtime/'observations.json').read_bytes())
    origins=binding.loaded(loaded,'8.0.27',False,'undo_store')
    for name in ['org.apache.seata.rm.datasource.undo.AbstractUndoLogManager','org.apache.seata.rm.datasource.undo.mysql.MySQLUndoLogManager']:
        need(Path(origins['bound_origins'][name]['actual_file'])==runtime/'classes'/(name.replace('.','/')+'.class'),'New original source shadowed')
    save('origin-binding.json',origins);binding.verify()
    for row in frozen:need(sha(Path(row['path']))==row['sha256'],'Java store peer input drift')
    save('result.json',dict(status='SELECTED_ORIGINAL_UNDO_GUARD_METHODS_MATCH',comparisons=2,original_newly_compiled_sources=2,actual_JDBC='8.0.27',actual_schema_and_database=env['SEATA_MYSQL_DATABASE'],
        archived_Rust_guard_reinserted_as_fixture=True,original_full_undo_invoked=False,original_JUnit_executed=False,SQL_Undo_commit_atomicity_verified=False,public_COMMIT_available=False,WP05_complete=False))
    save('catalog.json',dict(members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]));return dict(run=str(out),catalog_sha256=sha(out/'catalog.json'))
