"""Create a reviewable opt-in patch from the unchanged pinned export; never mutate that export."""
import difflib
import hashlib
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
ORIGINAL=Path('D:/codex-builds/seata-go-exact-20261112')
DEST=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498'


def sha(data):return hashlib.sha256(data).hexdigest()


def files():
    handler='pkg/remoting/getty/readwriter.go';model='pkg/protocol/message/other_message.go'
    old=(ORIGINAL/handler).read_bytes().replace(b'\r\n',b'\n').decode()
    new=old.replace('type RpcPackageHandler struct{}','// RpcPackageHandler 的 PayloadCodec 非 nil 时显式启用严格 Java V1 帧入口。\ntype RpcPackageHandler struct {\n\tPayloadCodec JavaV1PayloadCodec\n}')
    new=new.replace('func (p *RpcPackageHandler) Read(ss getty.Session, data []byte) (interface{}, int, error) {','func (p *RpcPackageHandler) Read(ss getty.Session, data []byte) (interface{}, int, error) {\n\tif p.PayloadCodec != nil { return p.readJavaV1(data) }')
    new=new.replace('func (p *RpcPackageHandler) Write(ss getty.Session, pkg interface{}) ([]byte, error) {','func (p *RpcPackageHandler) Write(ss getty.Session, pkg interface{}) ([]byte, error) {\n\tif p.PayloadCodec != nil { return p.writeJavaV1(pkg) }')
    new=new.replace('\ttotalLength := message.V1HeadLength','\tif msg.HeaderEntries != nil { return nil, fmt.Errorf("ordered/nullable headers require explicit Java V1 profile") }\n\n\ttotalLength := message.V1HeadLength')
    old_model=(ORIGINAL/model).read_bytes().replace(b'\r\n',b'\n').decode()
    new_model=old_model.replace('type RpcMessage struct {','type RpcMessage struct {\n\t// HeaderEntries 保留 Java V1 头字段顺序与 null；nil 使用原 HeadMap。\n\tHeaderEntries []JavaV1HeaderEntry `json:"headerEntries,omitempty"`')
    staged={handler:new.encode(),model:new_model.encode(),
        'pkg/remoting/getty/java_v1_profile_readwriter.go':(HERE/'java_v1_profile_readwriter.go').read_bytes(),
        'pkg/protocol/message/java_v1_header_entry.go':(HERE/'java_v1_header_entry.go').read_bytes()}
    result={}
    for path,data in staged.items():
        fmt=subprocess.run(['gofmt'],input=data,capture_output=True,check=True)
        result[path]=fmt.stdout
    return result


def run():
    content=files();parts=[];entries=[]
    for path,new in content.items():
        source=ORIGINAL/path;old=source.read_bytes().replace(b'\r\n',b'\n') if source.exists() else b''
        parts.append(f'diff --git a/{path} b/{path}\n')
        if not source.exists():parts.append('new file mode 100644\n')
        parts.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),new.decode().splitlines(keepends=True),fromfile='a/'+path if old else '/dev/null',tofile='b/'+path))
        entries.append(dict(path=path,original_sha256=sha(source.read_bytes()) if source.exists() else None,patched_sha256=sha(new)))
    patch=''.join(parts).encode();target=DEST/'0004-java-v1-getty-frame-profile.patch'
    target.write_bytes(patch)
    parents={name:sha((DEST/name).read_bytes()) for name in ['0001-java-v1-error-text-length.patch','0002-java-v1-response-exception-profile.patch','0003-java-v1-merged-payload-profile.patch']}
    (DEST/'getty_profile_manifest.json').write_text(json.dumps(dict(go_pin=DEST.name,patch=target.name,patch_sha256=sha(patch),requires=parents,files=entries,
        scope='Explicit handler injection only; default registry/client selection unchanged. Ordered nullable headers require explicit mode.'),indent=2)+'\n',encoding='utf-8')
    print(str(target))


if __name__=='__main__':run()
