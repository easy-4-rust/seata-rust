"""Original JAR negative control and isolated patched-class runtime with real SDK/HTTP probes."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import time

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def load_module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module

def export_overlay(build,directory,prepare):
    directory.mkdir(exist_ok=False);rows=[]
    for name,relative in prepare.FILES.items():
        if not name.endswith('.java'):continue
        package=Path(relative).relative_to('core/src/main/java').parent
        source_dir=build/'core/target/classes'/package;stem=Path(name).stem
        files=[source_dir/(stem+'.class')]+sorted(source_dir.glob(stem+'$*.class'))
        if not all(path.is_file() for path in files):raise RuntimeError('Missing freshly compiled candidate class: '+name)
        for source in files:
            target=directory/package/source.name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(source.read_bytes())
            rows.append(dict(path=target.relative_to(directory).as_posix(),sha256=sha(target),compiled_from=str(source)))
    (directory.parent/'overlay-manifest.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    return rows

def exact(stream,size):
    out=bytearray()
    while len(out)<size:
        data=stream.recv(size-len(out))
        if not data:raise RuntimeError('TC closed before expected frame')
        out.extend(data)
    return bytes(out)

def frame(stream):
    header=exact(stream,16);total=int.from_bytes(header[3:7],'big')
    if header[:3]!=b'\xda\xda\x01' or not 16<=total<=8*1024*1024:raise RuntimeError('Unexpected TC V1 frame')
    return header+exact(stream,total-16)

def heartbeat(identifier):return b'\xda\xda\x01'+struct.pack('>IHBBB i',16,16,3,1,0,identifier)

def expect_wait(stream):
    stream.settimeout(.12)
    try:
        value=stream.recv(1)
    except socket.timeout:return
    raise RuntimeError('Candidate did not wait for complete version prefix: '+value.hex())

def native_probes(directory,patched):
    result=[]
    with socket.create_connection(('127.0.0.1',28091),timeout=3) as stream:
        control=heartbeat(0x10203040);stream.sendall(control);reply=frame(stream)
        if reply[9]!=4 or reply[12:16]!=control[12:16]:raise RuntimeError('Original/modified TC heartbeat control differs')
        result.append(dict(name='complete-first-frame-control',request_hex=control.hex(),response_hex=reply.hex()))
    with socket.create_connection(('127.0.0.1',28091),timeout=3) as stream:
        stream.sendall(b'\xda')
        if not patched:
            try:closed=stream.recv(1)==b''
            except ConnectionResetError:closed=True
            if not closed:raise RuntimeError('Original negative behavior changed')
            result.append(dict(name='first-byte-rejected',fragment_hex='da',closed=True))
        else:
            expect_wait(stream);stream.sendall(b'\xda');expect_wait(stream)
            request=heartbeat(23)
            for value in request[2:]:stream.sendall(bytes([value]));time.sleep(.001)
            stream.settimeout(3);reply=frame(stream)
            if reply[9]!=4 or reply[12:16]!=request[12:16]:raise RuntimeError('Candidate fragmented heartbeat differs')
            result.append(dict(name='first-and-second-magic-byte-wait',request_hex=request.hex(),response_hex=reply.hex()))
            requests=[heartbeat(31),heartbeat(32)];stream.sendall(b''.join(requests));replies=[frame(stream),frame(stream)]
            if {int.from_bytes(row[12:16],'big') for row in replies}!={31,32} or any(row[9]!=4 for row in replies):raise RuntimeError('TC coalesced correlation differs')
            result.append(dict(name='two-coalesced-frames',responses=[row.hex() for row in replies]))
    if patched:
        with socket.create_connection(('127.0.0.1',28091),timeout=3) as stream:
            stream.sendall(b'\x00')
            try:closed=stream.recv(1)==b''
            except ConnectionResetError:closed=True
            if not closed:raise RuntimeError('Unknown prefix not rejected')
            result.append(dict(name='unknown-prefix-rejected',closed=True))
    (directory/'native-probes.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    return result

def http_probes(directory):
    path='/__preface_candidate_no_controller__'
    request=('GET '+path+' HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n').encode('ascii')
    with socket.create_connection(('127.0.0.1',28091),timeout=3) as stream:
        for value in request:stream.sendall(bytes([value]));time.sleep(.001)
        response=stream.recv(4096)
        if not response.startswith(b'HTTP/1.1 404'):raise RuntimeError('Fragmented HTTP1 did not reach normal no-controller response')
    preface=b'PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n'+b'\x00\x00\x00\x04\x00\x00\x00\x00\x00'
    with socket.create_connection(('127.0.0.1',28091),timeout=3) as stream:
        for value in preface:stream.sendall(bytes([value]));time.sleep(.001)
        header=exact(stream,9);length=int.from_bytes(header[:3],'big');payload=exact(stream,length)
        if header[3]!=4 or header[5:9]!=b'\x00'*4:raise RuntimeError('Fragmented HTTP2 did not produce server SETTINGS')
    proof=dict(http1_no_controller_status=404,http1_response_hex=response.hex(),http2_server_settings_hex=(header+payload).hex(),scope='Protocol selection/normal404 and HTTP2 SETTINGS only; not business HTTP/gRPC certification.')
    (directory/'http-probes.json').write_text(json.dumps(proof,indent=2)+'\n',encoding='utf-8');return proof

def class_sources(log,overlay,prepare):
    text=log.read_bytes().decode('utf-8','replace');rows=[]
    expected_url='file:/'+overlay.as_posix().rstrip('/')+'/'
    for name,relative in prepare.FILES.items():
        if not name.endswith('.java'):continue
        fqcn=relative.split('src/main/java/',1)[1][:-5].replace('/','.')
        matches=re.findall(r'\[class,load\s*\]\s+'+re.escape(fqcn)+r'\s+source:\s+(\S+)',text)
        if matches!=[expected_url]:raise RuntimeError('Actual candidate class origin differs: '+fqcn+' '+str(matches))
        rows.append(dict(class_name=fqcn,source=matches[0]))
    return rows

def run(out,build,go_cohort,sdk,prepare):
    h=sdk.h;l=sdk.l;c=sdk.c
    runtime=Path('D:/codex-builds')/('preface-live-'+time.strftime('%Y%m%d_%H%M%S')+'-'+out.name[-8:]);runtime.mkdir(exist_ok=False)
    if h.port_open(28091) or h.port_open(28092):raise RuntimeError('Owned preface ports already occupied')
    overlay=out/'overlay';overlay_rows=export_overlay(build,overlay,prepare)
    jar=c.ORACLE/'server/target/seata-server.jar'
    if h.sha(jar)!=h.JAR_HASH:raise RuntimeError('Original JAR drifted')
    states=[]
    for label,patched in [('original',False),('candidate',True)]:
        work=runtime/label;work.mkdir();directory=out/label;directory.mkdir()
        config=(ROOT/'tools/java-tc-integration/config/application.yml').read_text(encoding='utf-8').replace('18091','28091')
        h.write(work/'application.yml',config);h.write(directory/'application.yml',config)
        env=h.ENV.copy();env['SEATA_TC_WORK_DIR']=work.as_posix();tc=None;relay=None;success=False;proof=None
        args=[h.JAVA,'-Djava.io.tmpdir=D:/Temp','-Duser.home='+str(work),'-Dspring.config.location=file:'+(work/'application.yml').as_posix()]
        if patched:args+=['-verbose:class','-Dloader.path='+overlay.as_posix(),'-cp',jar,'org.springframework.boot.loader.launch.PropertiesLauncher']
        else:args+=['-jar',jar]
        args+=['-h','127.0.0.1']
        try:
            tc=h.spawn(args,directory,'tc',work,env);h.wait_until(lambda:h.port_open(28091),90,'Owned '+label+' preface TC startup',tc)
            native=native_probes(directory,patched)
            if not patched:
                h.wait_until(lambda:b'preface = [-38]' in (directory/'tc-stdout.log').read_bytes(),3,'Original single-byte rejection',tc)
                proof=dict(original_rejection_reproduced=True,native=native)
            else:
                http=http_probes(directory)
                # 仅派生测试转发策略：原SDK代理逻辑不变，去掉TC方向的整帧发送规避。
                source=(ROOT/'tools/go-java-v1-sdk-profile/proxy.py').read_bytes();needle=b'connection.backend.sendall(frame)'
                if source.count(needle)!=2:raise RuntimeError('Expected exact SDK proxy full-frame bypass identities')
                derived=b'# Test-only derived proxy: both directions fragmented; inherited whole-frame comments describe only the original policy.\n'+source.replace(needle,b'self.send(connection,connection.backend,frame)')
                derived_path=directory/'fragmenting-proxy.py';h.write(derived_path,derived)
                module=load_module('both_direction_preface_proxy',derived_path);relay=module.SdkProxy(work,directory);relay.start()
                binary=directory/'sdk-live.exe';l.run_command(['go','build','-o',binary,ROOT/'tools/go-java-v1-sdk-profile/live_main.go'],go_cohort,c.GO_ENV,directory/'go-build.log')
                result=subprocess.run([str(binary),str(work)],cwd=work,env=env,capture_output=True,creationflags=h.FLAGS,timeout=65)
                h.write(directory/'client.log',result.stdout+result.stderr);relay.close();relay=None
                if (work/'sdk.log').exists():h.write(directory/'sdk.log',(work/'sdk.log').read_bytes())
                rows=[json.loads(line.split(b'SDK_LIVE_PASS ',1)[1]) for line in result.stdout.splitlines() if line.startswith(b'SDK_LIVE_PASS ')]
                if result.returncode or len(rows)!=1:raise RuntimeError('Patched TC/SDK both-direction fragmentation failed')
                inventory=sdk.live.verify(directory,rows[0]);loaded=class_sources(directory/'tc-stdout.log',overlay,prepare)
                proof=dict(native=native,http=http,sdk=rows[0],sdk_wire=inventory,class_sources=loaded,sdk_binary_sha256=h.sha(binary),original_proxy_sha256=hashlib.sha256(source).hexdigest(),derived_proxy_sha256=h.sha(derived_path),forced_fragment_directions=['sdk-to-tc','tc-to-sdk'])
            success=True
        finally:
            if relay:relay.close()
            h.stop(tc);h.wait_until(lambda:not h.port_open(28091) and not h.port_open(28092),10,'Owned '+label+' preface cleanup')
            state=dict(profile=label,succeeded=success,tc_pid=tc.pid if tc else None,runtime_directory=str(work),ports_released=not h.port_open(28091) and not h.port_open(28092),original_jar_sha256=h.sha(jar),proof=proof)
            h.write(directory/'result.json',json.dumps(state,indent=2)+'\n');states.append(state)
    h.write(out/'runtime-result.json',json.dumps(dict(succeeded=all(s['succeeded'] for s in states),profiles=states,overlay=overlay_rows,scope='Original negative control plus independent overlay positive evidence; original artifact remains unfixed.'),indent=2)+'\n')
    return states
