"""用固定原Java编码/解码37帧，冻结来源后导出只增不覆盖的正式Rust夹具。"""
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[2]
ORACLE = Path('E:/workspaces/workspace-github-easy-4-rust/seata-java-phase09-oracle')
PIN = 'cba73e180fd8896a99df6415b859d275122feaa7'
JAR_SHA = 'aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5'
PARENT = ROOT / 'docs/phase-0/evidence/local-wp00-vector-audit/run_20261113_133856_5af18bca'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as stream:
        stream.write(data.encode('utf-8') if isinstance(data, str) else data)


def save(path, value):
    put(path, json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def generated_owner(name, source):
    """只识别实际JVM Lambda/CGLIB形态；返回必须另行绑定原class的owner，其余返回None。"""
    match = re.fullmatch(r'(org\.apache\.seata\.[\w.$]+)\$\$Lambda\$\d+/0x[0-9a-fA-F]+', name)
    if match and source == match[1]:
        return match[1]
    match = re.fullmatch(r'(org\.apache\.seata\.[\w.$]+?)\$\$(?:EnhancerByCGLIB|FastClassByCGLIB)\$\$[0-9a-fA-F]+', name)
    if match and source == '__JVM_LookupDefineClass__':
        return match[1]
    return None


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-1/evidence/formal-frame-java' / ('run_' + stamp)
    runtime = Path('D:/codex-builds') / ('formal_frames_' + stamp)
    out.mkdir(parents=True)
    runtime.mkdir()
    print('JAVA_FRAME_RUN ' + str(out), flush=True)
    env = os.environ | {'TEMP': 'D:/Temp', 'TMP': 'D:/Temp', 'PYTHONDONTWRITEBYTECODE': '1'}

    def command(args, log):
        result = subprocess.run([str(arg) for arg in args], cwd=runtime, env=env,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120,
            creationflags=subprocess.CREATE_NO_WINDOW)
        put(out / log, result.stdout)
        if result.returncode:
            raise RuntimeError(log + ': exit ' + str(result.returncode))
        return result.stdout

    jar = ORACLE / 'server/target/seata-server.jar'
    if sha(jar) != JAR_SHA or command(['git', '-C', ORACLE, 'rev-parse', 'HEAD'], 'java-head.log').decode().strip() != PIN:
        raise ValueError('Java pin/JAR differs')
    helper = ROOT / 'tools/protocol-fixtures/FormalFrameOracle.java'
    expected = ROOT / 'tools/go-golden-vectors/expected.json'
    sources = [Path(__file__), helper, expected, PARENT / 'classpath.txt', PARENT / 'dependencies-before.json']
    for relative in ['core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolEncoderV1.java',
            'core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolDecoderV1.java',
            'core/src/main/java/org/apache/seata/core/rpc/netty/v1/HeadMapSerializer.java',
            'core/src/main/java/org/apache/seata/core/protocol/RpcMessage.java']:
        sources.append(ORACLE / relative)
    command(['git', '-C', ORACLE, 'diff', '--exit-code', 'HEAD', '--'] +
        [path.relative_to(ORACLE) for path in sources if path.is_relative_to(ORACLE)], 'java-source-clean.log')
    frozen = []
    for path in sources:
        snapshot = 'sources/' + hashlib.sha256(str(path).encode()).hexdigest()[:20] + '.bin'
        put(out / snapshot, path.read_bytes())
        frozen.append({'path': str(path), 'sha256': sha(path), 'snapshot': snapshot})
    save(out / 'inputs.json', frozen)
    # 在冻结classpath及其来源文件后解析，避免启动前输入遗漏。
    cp = (PARENT / 'classpath.txt').read_text(encoding='utf-8')
    dependencies = json.loads((PARENT / 'dependencies-before.json').read_bytes())
    if {str(Path(p).resolve()) for p in cp.split(';')} != {str(Path(r['path']).resolve()) for r in dependencies}:
        raise ValueError('Classpath/dependency set differs')
    for row in dependencies:
        if row['kind'] != 'file' or sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Dependency differs: ' + row['path'])
    save(out / 'dependencies.json', dependencies)
    # 所有Seata生产依赖须为原可执行JAR内的相同二进制，禁止测试覆盖生产class。
    original_members = {}
    with zipfile.ZipFile(jar) as archive:
        for member in archive.namelist():
            if member.startswith('BOOT-INF/lib/') and member.endswith('.jar'):
                original_members[Path(member).name] = archive.read(member)
    for row in dependencies:
        path = Path(row['path'])
        if path.name.startswith('seata-') and path.read_bytes() != original_members.get(path.name):
            raise ValueError('Seata dependency is not original JAR member')
    classes = runtime / 'compiled'
    classes.mkdir()
    quoted = lambda arg: '"' + str(arg).replace('\\', '/') + '"'
    compile_args = ['-encoding', 'UTF-8', '-cp', cp, '-d', classes, helper]
    put(runtime / 'compile.args', '\n'.join(map(quoted, compile_args)) + '\n')
    java = Path('D:/Java/jdk-17.0.12/bin/java.exe')
    command([java, '-version'], 'java-version.log')
    command([java.parent / 'javac.exe', '@' + str(runtime / 'compile.args')], 'compile.log')
    compiled = list(classes.rglob('*.class'))
    if len(compiled) != 1:
        raise ValueError('Need exactly one diagnostic helper class')
    put(out / 'artifacts/FormalFrameOracle.class', compiled[0].read_bytes())
    compiled_sha = sha(compiled[0])
    loaded = runtime / 'classes.log'
    args = ['-Djava.io.tmpdir=D:/Temp', '-Xlog:class+load=info:file=classes.log',
        '-cp', str(classes) + ';' + cp, 'org.easy4rust.seata.oracle.FormalFrameOracle', expected, out]
    put(runtime / 'run.args', '\n'.join(map(quoted, args)) + '\n')
    put(out / 'compile.args', (runtime / 'compile.args').read_bytes())
    put(out / 'run.args', (runtime / 'run.args').read_bytes())
    command([java, '@' + str(runtime / 'run.args')], 'execution.log')
    put(out / 'classes.log', loaded.read_bytes())
    class_rows = []
    generated = []
    originals = {}
    for name, source in re.findall(r'\[class,load\] (\S+) source: (\S+)', loaded.read_text(encoding='utf-8')):
        if not name.startswith('org.apache.seata.'):
            continue
        if not source.startswith('file:'):
            owner = generated_owner(name, source)
            if owner is None:
                raise ValueError('Non-file production class is not an approved generated shape: ' + name + ' ' + source)
            generated.append({'class': name, 'source': source, 'owner': owner,
                              'kind': 'runtime_generated_not_original_classfile'})
            continue
        path = Path(unquote(urlparse(source).path).lstrip('/'))
        raw = original_members.get(path.name)
        if raw is None or path.read_bytes() != raw:
            raise ValueError('Executed Seata source is not original: ' + source)
        if path.name not in originals:
            originals[path.name] = zipfile.ZipFile(io.BytesIO(raw))
        data = originals[path.name].read(name.replace('.', '/') + '.class')
        digest = hashlib.sha256(data).hexdigest()
        snapshot = 'original-classes/' + digest[:24] + '.bin'
        if not (out / snapshot).exists():
            put(out / snapshot, data)
        class_rows.append({'class': name, 'source': str(path), 'sha256': digest, 'snapshot': snapshot})
    values = json.loads((out / 'java.json').read_bytes())
    identities = {row['class']: row['sha256'] for row in class_rows}
    if any(row['owner'] not in identities for row in generated):
        raise ValueError('Generated class owner is not bound to original JAR')
    for origin in values['origins']:
        if identities.get(origin['class']) != origin['sha256']:
            raise ValueError('Critical class identity differs')
    if len(values['frames']) != 37 or len({r['name'] for r in values['frames']}) != 37:
        raise ValueError('Incomplete frame set')
    for row in frozen:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Source drift')
    for row in dependencies:
        if sha(Path(row['path'])) != row['sha256']:
            raise ValueError('Dependency drift')
    if sha(jar) != JAR_SHA or sha(compiled[0]) != compiled_sha:
        raise ValueError('Execution artifact drift')
    save(out / 'original-classes.json', class_rows)
    save(out / 'generated-classes.json', generated)
    save(out / 'result.json', {'java_frames': 37, 'actual_original_encoder_decoder': True,
        'junit_engine': False, 'original_classes': len(class_rows), 'dependencies': len(dependencies),
        'source_pin': PIN, 'jar_sha256': JAR_SHA, 'helper_sha256': compiled_sha,
        'external_services_started': 0})
    members = [{'path': path.relative_to(out).as_posix(), 'sha256': sha(path)} for path in sorted(out.rglob('*')) if path.is_file()]
    save(out / 'catalog.json', {'members': members})
    target = ROOT / 'tests/fixtures/protocol/v1/formal_frames'
    data = (out / 'java.tsv').read_bytes()
    if (target / 'java.tsv').exists():
        if (target / 'java.tsv').read_bytes() != data:
            raise ValueError('Refuse to overwrite different fixture')
    else:
        put(target / 'java.tsv', data)
        save(target / 'manifest.json', {'source_pin': PIN, 'cases': 37, 'java_tsv_sha256': hashlib.sha256(data).hexdigest(),
            'evidence': out.relative_to(ROOT).as_posix(), 'catalog_sha256': sha(out / 'catalog.json')})
    certificate = target / 'certification.json'
    if not certificate.exists():
        save(certificate, {'source_pin': PIN, 'cases': 37, 'java_tsv_sha256': hashlib.sha256(data).hexdigest(),
            'evidence': out.relative_to(ROOT).as_posix(), 'catalog_sha256': sha(out / 'catalog.json'),
            'strict_generated_class_source_validation': True})
    print(json.dumps({'run': str(out), 'java_frames': 37, 'catalog_sha256': sha(out / 'catalog.json')}), flush=True)


if __name__ == '__main__':
    run()
