"""Recompile pinned original JSON JDBC/Undo source using sealed historical dependencies."""
import json, os, subprocess
from pathlib import Path
from inventory_rebuilt import ROOT, need, sha, save

PACKET=ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34'
DIGEST='b998a485e845359e05ad3a8b2edac389b8ba26b8cda990b5bde9f7ffb1f00bf0'
PIN='cba73e180fd8896a99df6415b859d275122feaa7'


def build(work,out,env,oracle):
    need(sha(PACKET/'catalog.json')==DIGEST,'Historical Java dependency packet changed')
    for row in json.loads((PACKET/'catalog.json').read_bytes())['members']:
        need(sha(PACKET/row['path'])==row['sha256'],'Historical Java dependency snapshot differs')
    inputs=json.loads((PACKET/'source-inputs.json').read_bytes()); old=json.loads((PACKET/'build.json').read_bytes())
    runtime=work/'temporal-oracle';runtime.mkdir(); evidence=out/'java-build';evidence.mkdir();sources=[];bindings=[];libraries=[];fallbacks=[]
    def put(path,data):path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
    def snapshot(row):
        data=(PACKET/row['snapshot']).read_bytes();need(sha(PACKET/row['snapshot'])==row['sha256'],'Frozen Java input differs');return data
    def bind(path,data,role):
        put(path,data);relative=path.relative_to(runtime).as_posix();put(evidence/'runtime'/relative,data);bindings.append(dict(path=str(path),relative=relative,sha256=sha(path),role=role))
    for original in old['explicit_sources']:
        p=Path(original)
        if 'seata-java-phase09-oracle' not in original:continue
        relative=p.relative_to(oracle).as_posix();value=subprocess.run(['git','-C',str(oracle),'show',PIN+':'+relative],capture_output=True,check=True,creationflags=subprocess.CREATE_NO_WINDOW).stdout
        target=runtime/'src'/relative;bind(target,value,'original_source');sources.append(target)
    helper=ROOT/'tools/formal-source-junit/TemporalColumnOracle.java';target=runtime/'src/TemporalColumnOracle.java';bind(target,helper.read_bytes(),'test_harness');sources.append(target)
    for item in old['libraries']:
        row=next(r for r in inputs if r['sha256']==item['sha256'] and Path(r['path']).name==Path(item['path']).name)
        target=runtime/'lib'/Path(item['path']).name;bind(target,snapshot(row),'library');libraries.append(target)
    driver=next(r for r in old['driver_libraries'] if r['version']=='8.0.27');row=next(r for r in inputs if r['sha256']==driver['sha256']);target=runtime/'lib/mysql-connector-java-8.0.27.jar';bind(target,snapshot(row),'driver');libraries.append(target)
    for i,original in enumerate([oracle/'rm-datasource/target/classes',oracle/'sqlparser/seata-sqlparser-core/target/classes']):
        original=Path(original);target=runtime/('fallback-'+str(i));fallbacks.append(target)
        for row in inputs:
            p=Path(row['path'])
            if p.is_relative_to(original):bind(target/p.relative_to(original),snapshot(row),'frozen_fallback')
    classes=runtime/'classes';classes.mkdir();cp=os.pathsep.join(map(str,libraries+fallbacks));argv=[Path('D:/Java/jdk-17.0.12/bin/javac.exe'),'-encoding','UTF-8','-cp',cp,'-d',classes,*sources]
    result=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env,capture_output=True,timeout=120,creationflags=subprocess.CREATE_NO_WINDOW);(evidence/'compile.stdout').write_bytes(result.stdout);(evidence/'compile.stderr').write_bytes(result.stderr);need(result.returncode==0,'Fresh original JSON Java compile failed: '+result.stderr[-2000:].decode(errors='replace'))
    for p in sorted(classes.rglob('*')):
        if p.is_file():
            relative=p.relative_to(runtime).as_posix();put(evidence/'runtime'/relative,p.read_bytes());bindings.append(dict(path=str(p),relative=relative,sha256=sha(p),role='fresh_class'))
    cp=os.pathsep.join(map(str,[classes,*libraries,*fallbacks]));save(evidence/'binding.json',dict(java_pin=PIN,historical_packet=str(PACKET),historical_catalog_sha256=DIGEST,dependency_profile='original selected Jackson 2.18.3 / Connector-J 8.0.27; frozen helper fallback, not full Maven rebuild',classpath=cp,compile_argv=list(map(str,argv)),compile_exit_code=result.returncode,files=bindings));return cp


def verify(out):
    binding=json.loads((out/'java-build/binding.json').read_bytes());need(binding['java_pin']==PIN and binding['historical_catalog_sha256']==DIGEST and binding['compile_exit_code']==0,'Original Java binding differs')
    for row in binding['files']:
        need(sha(Path(row['path']))==row['sha256']==sha(out/'java-build/runtime'/row['relative']),'Executed JSON Java classpath drift')
