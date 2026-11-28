from pathlib import Path
import os,subprocess,json,hashlib
root=Path(__file__).resolve().parents[2];out=root/'docs/phase-2/evidence/mysql-exact-temporal-adapter/reader_checks_20261128';out.mkdir(exist_ok=True)
work=Path('D:/codex-builds/seata-exact-reader-checks-20261128');(work/'classes').mkdir(parents=True,exist_ok=True)
driver=Path('D:/codex-builds/seata-exact-temporal-20261128_103207_a19bc560/exact-temporal-oracle/lib/mysql-connector-java-8.0.27.jar')
sources=[root/'compat/java/src/main/java/io/github/easy4j/seata/rust/jdbc/ExactTemporalValueReader.java',root/'compat/java/src/test/java/io/github/easy4j/seata/rust/jdbc/ExactTemporalValueReaderTest.java']
env=os.environ|{'TEMP':'D:/Temp','TMP':'D:/Temp'};commands=[]
for name,argv in [('compile',['D:/Java/jdk-17.0.12/bin/javac.exe','-encoding','UTF-8','-cp',str(driver),'-d',str(work/'classes'),*map(str,sources)]),('run',['D:/Java/jdk-17.0.12/bin/java.exe','-cp',str(work/'classes')+os.pathsep+str(driver),'io.github.easy4j.seata.rust.jdbc.ExactTemporalValueReaderTest'])]:
 r=subprocess.run(argv,env=env,cwd=root,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);commands.append(dict(argv=argv,exit_code=r.returncode));(out/'commands.json').write_text(json.dumps(commands,indent=2),encoding='utf-8');print(name,r.returncode);print((r.stdout+r.stderr).decode(errors='replace')[-1800:]);assert r.returncode==0
(out/'sources.json').write_text(json.dumps([dict(path=p.relative_to(root).as_posix(),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sources],indent=2),encoding='utf-8')

import shutil
for source in sources:
 target=out/'source'/source.relative_to(root);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
for source in (work/'classes').rglob('*.class'):
 target=out/'classes'/source.relative_to(work/'classes');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
shutil.copyfile(Path(__file__),out/Path(__file__).name)
(out/'driver.json').write_text(json.dumps(dict(path=str(driver),sha256=hashlib.sha256(driver.read_bytes()).hexdigest()),indent=2),encoding='utf-8')
(out/'catalog.json').write_text(json.dumps([dict(path=p.relative_to(out).as_posix(),sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json'],indent=2),encoding='utf-8')
