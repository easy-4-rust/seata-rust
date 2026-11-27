"""Compile unchanged pinned Java XA classes and seal emitted binary identity vectors."""
import datetime,json,os,shutil,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-4/evidence/mysql-xa'/('java_'+stamp);out.mkdir(parents=True)
    work=Path('D:/codex-builds')/('seata-xa-oracle-'+stamp);classes=work/'classes';classes.mkdir(parents=True)
    oracle=ROOT.parent/'seata-java-phase09-oracle';pin='cba73e180fd8896a99df6415b859d275122feaa7';paths=[]
    for name in ['XAXid','XABranchXid','XAXidBuilder']:
        name='rm-datasource/src/main/java/org/apache/seata/rm/datasource/xa/'+name+'.java'
        raw=subprocess.check_output(['git','show',pin+':'+name],cwd=oracle)
        dest=work/Path(name).name;dest.write_bytes(raw);paths.append(dest);(out/dest.name).write_bytes(raw)
    wrapper=ROOT/'tools/xa/java/XidOracle.java';shutil.copyfile(wrapper,work/wrapper.name);shutil.copyfile(wrapper,out/wrapper.name);paths.append(work/wrapper.name)
    commands=[]
    for name,argv in [('compile',['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-d',str(classes),*map(str,paths)]),('oracle',['D:/Java/jdk-17.0.12/bin/java.exe','-cp',str(classes),'XidOracle'])]:
        value=subprocess.run(argv,cwd=ROOT,env=os.environ|dict(TEMP="D:/Temp",TMP="D:/Temp"),capture_output=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(name+'.stdout')).write_bytes(value.stdout);(out/(name+'.stderr')).write_bytes(value.stderr);commands.append(dict(label=name,argv=argv,exit_code=value.returncode));need(value.returncode==0,'Original XA Java oracle failed')
    need(len(value.stdout.splitlines())==8,'Original Java vector count differs')
    fixture=ROOT/'crates/seata_xa/tests/fixtures/java_xid.tsv';fixture.parent.mkdir(parents=True,exist_ok=True);fixture.write_bytes(value.stdout)
    save(out/'commands.json',commands);save(out/'result.json',dict(status='PINNED_JAVA_XA_ORIGINAL_OBJECTS_EXECUTED',Java_pin=pin,vectors=8,fixture=fixture.relative_to(ROOT).as_posix(),fixture_sha256=sha(fixture),input_sources=[dict(name=p.name,sha256=sha(p)) for p in paths]))
    shutil.copyfile(Path(__file__),out/Path(__file__).name);save(out/'catalog.json',[dict(path=p.name,sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()]);print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))))
if __name__=='__main__':run()
