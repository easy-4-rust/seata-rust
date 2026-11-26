from pathlib import Path
import hashlib, json, os, shutil, subprocess
from urllib.parse import urlparse
from urllib.request import url2pathname

ROOT = Path.cwd()
PIN = 'cba73e180fd8896a99df6415b859d275122feaa7'
JAVA_REPO = ROOT.parent / 'seata-java-phase09-oracle'
OUT = ROOT / 'docs/phase-2/evidence/sqlparser-escape-dialects/run_20261118_v3'
WORK = ROOT.parent / '.seata-escape-dialects-oracle-v3'
SERVICES = 'META-INF/services/org.apache.seata.sqlparser.EscapeHandler'
PREFIX = 'rm-datasource/src/main/'
JDK = Path('D:/Java/jdk-17.0.12')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def blob(relative):
    return subprocess.check_output(['git', '-C', str(JAVA_REPO), 'cat-file', 'blob', PIN + ':' + relative])

def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n', 'utf-8')

def run():
    OUT.mkdir(parents=True)
    classes = WORK / 'classes'
    classes.mkdir(parents=True)
    services = classes / SERVICES
    services.parent.mkdir(parents=True)
    services.write_bytes(blob(PREFIX + 'resources/' + SERVICES))
    shutil.copy2(services, OUT / 'providers.txt')
    names = [line for line in services.read_text('utf-8').splitlines() if line and not line.startswith('#')]
    assert len(names) == len(set(names)) == 10
    sources = {}
    for name in names:
        relative = PREFIX + 'java/' + name.replace('.', '/') + '.java'
        original = blob(relative)
        path = OUT / (name.split('.')[-1] + '.java')
        path.write_bytes(original)
        sources[relative] = sha(path)
    oracle = OUT / 'EscapeDialectsOracle.java'
    shutil.copy2(ROOT / 'tools/formal-sqlparser-core' / oracle.name, oracle)

    base = [
        JAVA_REPO / 'sqlparser/seata-sqlparser-core/target/classes',
        JAVA_REPO / 'common/target/classes',
        Path('C:/Users/hnxyh/.m2/repository/org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar'),
        Path('C:/Users/hnxyh/.m2/repository/org/apache/commons/commons-lang3/3.18.0/commons-lang3-3.18.0.jar'),
    ]
    inputs = {str(p):sha(p) for p in [JDK/'bin/java.exe', JDK/'bin/javac.exe', JDK/'lib/modules', JDK/'release']}
    for entry in base:
        if entry.is_dir():
            inputs.update({str(p):sha(p) for p in entry.rglob('*') if p.is_file()})
        else:
            inputs[str(entry)] = sha(entry)
    cp = os.pathsep.join(str(p) for p in [classes, *base])
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp')
    for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS'):
        env.pop(key, None)
    commands = []
    target = OUT/'java.tsv'
    definitions = OUT/'definitions.tsv'
    for name, argv in [
        ('compile', [JDK/'bin/javac.exe','-encoding','UTF-8','-cp',cp,'-d',classes,*sorted(OUT.glob('*.java'))]),
        ('java', [JDK/'bin/java.exe','-Duser.home='+str(WORK/'home'),'-cp',cp,'EscapeDialectsOracle',target,definitions]),
    ]:
        argv = list(map(str,argv))
        with (OUT/(name+'.log')).open('wb') as log:
            code = subprocess.run(argv,cwd=WORK,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120).returncode
        commands.append(dict(argv=argv,cwd=str(WORK),exit_code=code))
        save(OUT/'commands.json',commands)
        assert code==0,(OUT/(name+'.log')).read_text('utf-8')
    assert all(sha(Path(path))==digest for path,digest in inputs.items())
    descriptor_rows = definitions.read_text('utf-8').splitlines()
    assert len(descriptor_rows) == 10
    for row in descriptor_rows:
        url = urlparse(row.split('\t')[-1])
        assert url.scheme == 'file' and Path(url2pathname(url.path)).resolve() == classes.resolve()
    for path in classes.rglob('*.class'):
        archive = OUT/'classes'/path.relative_to(classes)
        archive.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(path,archive)
    rows=target.read_text('utf-8').splitlines()
    words={}
    for row in rows:
        fields=row.split('\t')
        if fields[1]=='keyword':
            words.setdefault(fields[0],[]).append(fields[2])
    mysql=ROOT/'crates/seata_db_mysql/src/mysql/mysql_keywords.txt'
    for name in ('mysql','mariadb','polardb-x'):
        assert words[name]==mysql.read_text('utf-8').splitlines()
    for name, entries in words.items():
        if name in ('mysql','mariadb','polardb-x'):
            continue
        directory=ROOT/'crates/seata_db_mysql/src'/name
        directory.mkdir(exist_ok=True)
        target_words=directory/(name+'_keywords.txt')
        data = ('\n'.join(entries)+'\n').encode('utf-8')
        if target_words.exists():
            assert target_words.read_bytes() == data
        else:
            target_words.write_bytes(data)
    fixture=ROOT/'crates/seata_db_mysql/tests/fixtures/escape-dialects-java-v3.tsv'
    assert not fixture.exists()
    fixture.write_bytes(target.read_bytes())
    save(OUT/'oracle-result.json',dict(pin=PIN,records=len(rows),keyword_counts={k:len(v) for k,v in words.items()},
        production_provider_sources=sources,descriptors_sha256=sha(services),
        output_sha256=sha(target),oracle_sha256=sha(oracle),runtime_inputs=inputs))
    print(len(rows),{k:len(v) for k,v in words.items()})

if __name__=='__main__':
    run()
