"""Derive0006 only from audited SDK0005 bytes; never edit earlier templates/cohorts."""
import difflib
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('inflight_parent_prepare',ROOT/'tools/go-java-v1-sdk-profile/prepare_candidate.py');parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
DEST=parent.DEST;PATCH=DEST/'0006-java-v1-inflight-outcomes.patch';MANIFEST=DEST/'inflight_manifest.json'
def sha(data):return hashlib.sha256(data).hexdigest()
def files():
 base=parent.files();paths=['pkg/remoting/getty/getty_remoting.go','pkg/remoting/getty/getty_client.go','pkg/remoting/getty/listener.go','pkg/remoting/getty/session_manager.go','pkg/remoting/processor/client/client_on_response_processor.go']
 data={path:base[path].decode() if path in base else (parent.ORIGINAL/path).read_text(encoding='utf-8') for path in paths}
 path=paths[0];data[path]=data[path].replace('type (\n','type (\n',1).replace('GettyRemoting  struct {','GettyRemoting  struct {\n javaV1Pending sync.Map // 显式同步请求的会话归属与阶段。')
 # gofmt版本可能改变类型对齐，因此使用精确原始签名定位字段。
 if 'javaV1Pending sync.Map' not in data[path]:data[path]=data[path].replace('GettyRemoting struct {','GettyRemoting struct {\n javaV1Pending sync.Map')
 data[path]=data[path].replace('javaV1Pending sync.Map','javaV1StateMutex sync.Mutex // 仅保护登记/快照/清理，不持锁处理消息或执行网络。\n javaV1Pending sync.Map')
 signature='func (g *GettyRemoting) SendSync(msg message.RpcMessage, s getty.Session, callback callbackMethod) (interface{}, error) {'
 assert data[path].count(signature)==1
 data[path]=data[path].replace(signature,signature+'\n if javaV1TransportEnabled()&&msg.Type==message.RequestTypeRequestSync{return g.sendJavaV1Sync(msg,s,callback)}')
 path=paths[1];start=data[path].index('func (client *GettyRemotingClient) SendSyncRequest(');before,tail=data[path][:start],data[path][start:]
 tail=tail.replace('return nil, err','if javaV1TransportEnabled(){return nil,javaV1LocalError(0,err)}\n return nil, err',1);data[path]=before+tail
 data[path]=data[path].replace('defer g.gettyRemoting.RemoveMessageFuture(reqMsg.ID)','if javaV1TransportEnabled(){defer g.gettyRemoting.futures.CompareAndDelete(reqMsg.ID,respMsg)}else{defer g.gettyRemoting.RemoveMessageFuture(reqMsg.ID)}')
 path=paths[2];sig='func (g *gettyClientHandler) OnError(session getty.Session, err error) {'
 data[path]=data[path].replace(sig,sig+'\n if javaV1TransportEnabled()&&gettyRemotingClient!=nil{gettyRemotingClient.gettyRemoting.failJavaV1Session(session,err)}')
 for sig in ['func (g *gettyClientHandler) OnError(session getty.Session, err error) {','func (g *gettyClientHandler) OnClose(session getty.Session) {']:
  start=data[path].index(sig);end=data[path].index('\n}',start)
  cause='err' if 'OnError' in sig else 'errJavaV1ConnectionClosed'
  data[path]=data[path][:end]+'\n if javaV1TransportEnabled()&&gettyRemotingClient!=nil{gettyRemotingClient.gettyRemoting.failJavaV1Session(session,'+cause+')}'+data[path][end:]
 needle='if err := GetGettyRemotingClient().checkJavaV1Envelope(rpcMessage); err != nil {'
 data[path]=data[path].replace(needle,'var accepted bool\n ctx,accepted=GetGettyRemotingClient().bindJavaV1Response(ctx,session,rpcMessage);if !accepted{return}\n if _,bound:=ctx.Value(javaV1ResponseGuardKey{}).(*javaV1ResponseGuard);bound&&javaV1ReplyEnvelopeError(rpcMessage)!=nil{_,_=GetGettyRemotingClient().HandleJavaV1BoundResponse(ctx,rpcMessage);return}\n '+needle)
 path=paths[3];needle='atomic.AddInt32(&g.sessionSize, -1)';assert data[path].count(needle)==1
 data[path]=data[path].replace(needle,'if g.javaV1Profile&&gettyRemotingClient!=nil{gettyRemotingClient.gettyRemoting.failJavaV1Session(session,errJavaV1ConnectionClosed)}\n '+needle)
 path=paths[4];data[path]=data[path].replace('return f.handleSeataOnResponse(rpcMessage)','if handled,e:=getty.GetGettyRemotingClient().HandleJavaV1BoundResponse(ctx,rpcMessage);handled{return e}\n return f.handleSeataOnResponse(rpcMessage)')
 content={path:value.encode() for path,value in data.items()}
 for name in ['java_v1_inflight.go','java_v1_transport_error.go','java_v1_response_guard.go']:content['pkg/remoting/getty/'+name]=(HERE/name).read_bytes()
 return {path:subprocess.run(['gofmt'],input=value,capture_output=True,check=True).stdout for path,value in content.items()}
def run():
 base=parent.files();content=files();rows=[];parts=[]
 for path,new in content.items():
  old=base.get(path,(parent.ORIGINAL/path).read_bytes().replace(b'\r\n',b'\n') if (parent.ORIGINAL/path).exists() else b'')
  parts.append(f'diff --git a/{path} b/{path}\n')
  if not old:parts.append('new file mode 100644\n')
  parts.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),new.decode().splitlines(keepends=True),fromfile='a/'+path if old else '/dev/null',tofile='b/'+path))
  rows.append(dict(path=path,parent_sha256=sha(old) if old else None,patched_sha256=sha(new)))
 data=''.join(parts).encode();PATCH.write_bytes(data);MANIFEST.write_text(json.dumps(dict(go_pin=DEST.name,patch=PATCH.name,patch_sha256=sha(data),requires={p.name:sha(p.read_bytes()) for p in sorted(DEST.glob('000[1-5]-*.patch'))},files=rows,scope='Explicit synchronous SDK Java V1 inflight outcomes/session ownership; no transaction resend, default unchanged.'),indent=2)+'\n',encoding='utf-8')
 print(PATCH)
if __name__=='__main__':run()
