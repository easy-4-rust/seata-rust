"""Compile unchanged pinned TCC context objects using dependencies from the pinned server artifact."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile
from inventory_rebuilt import ROOT, need, save, sha
from xa_managed_java_faults import guard


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out = ROOT/'docs/phase-4/evidence/tcc-core'/('java-util_'+stamp)
    out.mkdir(parents=True)
    work = Path('D:/codex-builds')/('seata-tcc-oracle-'+stamp)
    classes, libs = work/'classes', work/'libs'
    classes.mkdir(parents=True)
    libs.mkdir()
    (classes/'registry.conf').write_bytes(b'config {\n type = "file"\n file { name = "file.conf" }\n}\n')
    (classes/'file.conf').write_bytes(b'client { rm {} }\n')
    for name in ['registry.conf', 'file.conf']:
        shutil.copyfile(classes/name, out/name)
    jar = guard.reg.ORACLE/'server/target/seata-server.jar'
    need(sha(jar) == guard.reg.JAR_SHA, 'Pinned Java artifact changed')
    with zipfile.ZipFile(jar) as archive:
        for name in archive.namelist():
            # 独立客户端参考进程没有 Spring 容器；不装载服务器专用 Boot ConfigurationProvider。
            if name.startswith('BOOT-INF/lib/') and name.endswith('.jar') and Path(name).name != 'seata-spring-autoconfigure-core-2.8.0-SNAPSHOT.jar':
                (libs/Path(name).name).write_bytes(archive.read(name))
    for module in ['integration-tx-api', 'rm']:
        dependency = guard.reg.ORACLE/module/'target'/('seata-'+module+'-2.8.0-SNAPSHOT.jar')
        shutil.copyfile(dependency, libs/dependency.name)
    save(out/'dependencies.json', [dict(name=p.name, sha256=sha(p)) for p in sorted(libs.glob('*.jar'))])
    inputs = []
    decision_source = subprocess.check_output(['git', '-C', str(guard.reg.ORACLE), 'show', guard.reg.PIN+':spring/seata-spring/src/main/java/org/apache/seata/rm/fence/SpringFenceHandler.java'])
    (out/'SpringFenceHandler.java').write_bytes(decision_source)
    for name in ['integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContext.java',
                 'integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContextUtil.java',
                 'integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/interceptor/ActionContextUtil.java',
                 'integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/fence/constant/CommonFenceConstant.java']:
        raw = subprocess.check_output(['git', '-C', str(guard.reg.ORACLE), 'show', guard.reg.PIN+':'+name])
        p = work/Path(name).name
        p.write_bytes(raw)
        shutil.copyfile(p, out/p.name)
        inputs.append(p)
    wrapper = ROOT/'tools/tcc/java/UtilOracle.java'
    shutil.copyfile(wrapper, out/wrapper.name)
    inputs.append(wrapper)
    commands = []
    env = os.environ | dict(TEMP='D:/Temp', TMP='D:/Temp')
    for key in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'SEATA_CONFIG_NAME', 'SEATA_ENV']:
        env.pop(key, None)
    for label, argv in [
        ('compile', ['D:/Java/jdk-17.0.12/bin/javac.exe', '-encoding', 'UTF-8', '-cp', str(libs/'*'), '-d', str(classes), *map(str, inputs)]),
        ('oracle', ['D:/Java/jdk-17.0.12/bin/java.exe', '-Djava.io.tmpdir=D:/Temp', '-Duser.home='+str(work), '-Xlog:class+load=info:file='+str(work/'class-load.log'), '-cp', str(classes)+';'+str(libs/'*'), 'UtilOracle']),
    ]:
        value = subprocess.run(argv, cwd=work, env=env, capture_output=True, timeout=60, creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+'.stdout')).write_bytes(value.stdout)
        (out/(label+'.stderr')).write_bytes(value.stderr)
        commands.append(dict(label=label, argv=argv, exit_code=value.returncode))
        save(out/'commands.json', commands)
        need(value.returncode == 0, 'Pinned Java TCC oracle failed: '+value.stderr[-1600:].decode(errors='replace'))
    rows = [json.loads(line.split(b'\t',1)[1]) for line in value.stdout.splitlines() if line.startswith(b'UTIL\t')]
    need(len(rows) == 19, 'Actual mutation vector count changed')
    fixture = ROOT/'crates/seata_tcc/tests/fixtures/java_util.json'
    fixture.write_bytes(json.dumps(rows,ensure_ascii=False,indent=2).encode()+b'\n')
    shutil.copyfile(work/'class-load.log', out/'class-load.log')
    origins = (work/'class-load.log').read_bytes()
    for name in ['BusinessActionContext', 'BusinessActionContextUtil']:
        need(any(('org.apache.seata.rm.tcc.api.'+name+' source:').encode() in line and b'/classes/' in line for line in origins.splitlines()), 'TCC context did not load newly compiled original source')
    save(out/'result.json', dict(status='PINNED_JAVA_TCC_UTIL_EXECUTED', Java_pin=guard.reg.PIN, Java_jar_sha256=guard.reg.JAR_SHA, vectors=19, fixture=fixture.relative_to(ROOT).as_posix(), fixture_sha256=sha(fixture), original_sources=[dict(name=p.name, sha256=sha(p)) for p in inputs], TCC_gate_complete=False, whole_plan_complete=False))
    shutil.copyfile(Path(__file__), out/Path(__file__).name)
    save(out/'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out/'catalog.json'))))


if __name__ == '__main__':
    run()
