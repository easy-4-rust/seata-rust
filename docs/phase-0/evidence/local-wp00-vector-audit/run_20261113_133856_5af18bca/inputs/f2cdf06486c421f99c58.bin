"""Create0005 from pinned source without mutating previous candidate cohorts."""
import difflib
import hashlib
import json
import importlib.util
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
ORIGINAL=Path('D:/codex-builds/seata-go-exact-20261112')
DEST=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498'
def sha(data):return hashlib.sha256(data).hexdigest()
def files():
    paths=['pkg/remoting/config/config.go','pkg/remoting/getty/session_manager.go','pkg/remoting/getty/getty_client.go','pkg/remoting/processor/client/client_on_response_processor.go','pkg/remoting/getty/listener.go','pkg/remoting/getty/getty_init.go']
    data={path:(ORIGINAL/path).read_bytes().replace(b'\r\n',b'\n').decode() for path in paths}
    path=paths[0];data[path]=data[path].replace('type Config struct {','type Config struct {\n\t// JavaV1Profile 显式启用Java V1严格适配，默认关闭；manager创建时捕获。\n\tJavaV1Profile bool `yaml:"java-v1-profile" json:"java-v1-profile" koanf:"java-v1-profile"`')
    data[path]=data[path].replace('func (cfg *Config) RegisterFlagsWithPrefix(prefix string, f *flag.FlagSet) {','func (cfg *Config) RegisterFlagsWithPrefix(prefix string, f *flag.FlagSet) {\n\tf.BoolVar(&cfg.JavaV1Profile, prefix+".java-v1-profile", false, "Explicit Java V1 compatibility profile.")')
    path=paths[1];data[path]=data[path].replace('type SessionManager struct {','type SessionManager struct {\n\tjavaV1Profile bool // 初始化时捕获，重连不重新读取调用方选择。')
    data[path]=data[path].replace('onceSessionManager.Do(func() {','onceSessionManager.Do(func() {\n\t\t\tif gettyConfig.JavaV1Profile { snapshot:=*gettyConfig;gettyConfig=&snapshot;seataSnapshot:=*seataConfig;seataConfig=&seataSnapshot }')
    data[path]=data[path].replace('gettyConf:      gettyConfig,','gettyConf:      gettyConfig,\n\t\t\t\tjavaV1Profile: gettyConfig.JavaV1Profile,')
    data[path]=data[path].replace('func (g *SessionManager) setSessionConfig(session getty.Session) {','func (g *SessionManager) setSessionConfig(session getty.Session, handler getty.ReadWriter) {')
    data[path]=data[path].replace('session.SetPkgHandler(rpcPkgHandler)','session.SetPkgHandler(handler)').replace('g.setSessionConfig(session)','g.setSessionConfig(session, handler)')
    data[path]=data[path].replace('func (g *SessionManager) newSession(session getty.Session) error {','func (g *SessionManager) newSession(session getty.Session) error {\n\thandler, profileErr := g.sessionPackageHandler()\n\tif profileErr != nil { return profileErr }')
    data[path]=data[path].replace('g.startGettyClient(instance)','g.startGettyClient(instance,entry)').replace('func (g *SessionManager) startGettyClient(instance *discovery.ServiceInstance) getty.Client {','func (g *SessionManager) startGettyClient(instance *discovery.ServiceInstance,entry *serverClientEntry) getty.Client {')
    data[path]=data[path].replace('go gettyClient.RunEventLoop(g.newSession)','if g.javaV1Profile { go gettyClient.RunEventLoop(func(s getty.Session)error{s.SetAttribute(javaV1OwnerKey,entry);return g.newSession(s)}) } else { go gettyClient.RunEventLoop(g.newSession) }')
    data[path]=data[path].replace('atomic.AddInt32(&g.sessionSize, -1)','atomic.AddInt32(&g.sessionSize, -1)\n\tif g.javaV1Profile { if owner,ok:=session.GetAttribute(javaV1OwnerKey).(*serverClientEntry);ok {go g.reconnectJavaV1(session.RemoteAddr(),owner)} }')
    path=paths[2];data[path]=data[path].replace('func (client *GettyRemotingClient) SendAsyncRequest(msg interface{}) error {','func (client *GettyRemotingClient) SendAsyncRequest(msg interface{}) error {\n\tplan, err := javaV1PlanFor(msg)\n\tif err != nil { return err }')
    data[path]=data[path].replace('type GettyRemotingClient struct {','type GettyRemotingClient struct {\n\tmergeReplyMutex sync.Mutex // 同一批结果只允许一个处理器分发，包含失败路径。')
    data[path]=data[path].replace('client.gettyRemoting.SendAsync(rpcMessage, nil, client.asyncCallback)','client.gettyRemoting.SendAsync(rpcMessage, nil, client.javaV1Callback(plan,true))')
    data[path]=data[path].replace('func (client *GettyRemotingClient) SendSyncRequest(msg interface{}) (interface{}, error) {','func (client *GettyRemotingClient) SendSyncRequest(msg interface{}) (interface{}, error) {\n\tplan, err := javaV1PlanFor(msg)\n\tif err != nil { return nil,err }')
    data[path]=data[path].replace('client.gettyRemoting.SendSync(rpcMessage, nil, client.syncCallback)','client.gettyRemoting.SendSync(rpcMessage, nil, client.javaV1Callback(plan,false))')
    path=paths[3];before,tail=data[path].split('func (f *clientOnResponseProcessor) handleSeataOnResponse',1)
    start=tail.index('\tif mergedResult, ok := rpcMessage.Body.(message.MergeResultMessage); ok {')
    end=tail.index('\t} else {\n\t\t// 如果是请求消息，做处理逻辑',start)
    tail=tail[:start]+'\tif _, ok := rpcMessage.Body.(message.MergeResultMessage); ok {\n\t\treturn gettyRemotingClient.HandleJavaV1MergedResponse(rpcMessage)\n'+tail[end:]
    data[path]=before+'func (f *clientOnResponseProcessor) handleSeataOnResponse'+tail
    data[path]=data[path].replace('getty.GetGettyClientHandlerInstance().RegisterProcessor(message.MessageTypeSeataMergeResult, clientOnResponseProcessor)','getty.GetGettyClientHandlerInstance().RegisterProcessor(message.MessageTypeBranchCommitResult, clientOnResponseProcessor)\n\t\tgetty.GetGettyClientHandlerInstance().RegisterProcessor(message.MessageTypeBranchRollbackResult, clientOnResponseProcessor)\n\t\tgetty.GetGettyClientHandlerInstance().RegisterProcessor(message.MessageTypeSeataMergeResult, clientOnResponseProcessor)')
    data[path]=data[path].replace('func (f *clientOnResponseProcessor) handleSeataOnResponse(rpcMessage message.RpcMessage) error {','func (f *clientOnResponseProcessor) handleSeataOnResponse(rpcMessage message.RpcMessage) error {\n\tif rpcMessage.Type != message.RequestTypeResponse { return getty.GetGettyRemotingClient().RejectJavaV1Response(rpcMessage.ID,"not a response envelope") }')
    path=paths[4];data[path]=data[path].replace('ctx := context.Background()','ctx := context.Background()',1)
    data[path]=data[path].replace('\tif mm, ok := rpcMessage.Body.(message.MessageTypeAware); ok {','\tif err := GetGettyRemotingClient().checkJavaV1Envelope(rpcMessage); err != nil { log.Errorf("invalid Java V1 response: %v",err); return }\n\n\tif mm, ok := rpcMessage.Body.(message.MessageTypeAware); ok {')
    data[path]=data[path].replace('processor.Process(ctx, rpcMessage)','if err := processor.Process(ctx, rpcMessage); err != nil { log.Errorf("response processor failed: %v",err) }')
    path=paths[5];data[path]=data[path].replace('"seata.apache.org/seata-go/v2/pkg/remoting/config"','"seata.apache.org/seata-go/v2/pkg/remoting/config"\n"seata.apache.org/seata-go/v2/pkg/util/log"')
    data[path]=data[path].replace('func InitGetty(gettyConfig *config.Config, seataConfig *config.SeataConfig) {','func InitGetty(gettyConfig *config.Config, seataConfig *config.SeataConfig) {\n\tif gettyConfig==nil||seataConfig==nil {log.Errorf("invalid Getty initialization configuration");return}\n\tif gettyConfig.JavaV1Profile&&gettyConfig.SessionConfig.CompressEncoding {log.Errorf("Java V1 profile requires Getty compression disabled");return}')
    spec=importlib.util.spec_from_file_location('sdk_getty_base',ROOT/'tools/go-java-v1-getty-profile/prepare_candidate.py');base4=importlib.util.module_from_spec(spec);spec.loader.exec_module(base4)
    model='pkg/protocol/message/other_message.go';data[model]=base4.files()[model].decode()
    data[model]=data[model].replace('func (f *MessageFuture) Complete(response interface{}) bool {','func (f *MessageFuture) Complete(response interface{}) bool {\n\treturn f.CompleteWithError(response,nil)\n}\n\n// CompleteWithError 使用同一CAS原子发布Response与Err，然后唤醒等待者。\nfunc (f *MessageFuture) CompleteWithError(response interface{},err error) bool {')
    data[model]=data[model].replace('\tf.Response = response','\tf.Response = response\n\tf.Err = err')
    content={path:value.encode() for path,value in data.items()}
    for name in ['java_v1_reply_plan.go','java_v1_session_profile.go','java_v1_unknown_reply_error.go']:content['pkg/remoting/getty/'+name]=(HERE/name).read_bytes()
    return {path:subprocess.run(['gofmt'],input=value,capture_output=True,check=True).stdout for path,value in content.items()}
def run():
    content=files();parts=[];rows=[]
    for path,new in content.items():
        source=ORIGINAL/path;old=source.read_bytes().replace(b'\r\n',b'\n') if source.exists() else b''
        if path=='pkg/protocol/message/other_message.go':
            spec=importlib.util.spec_from_file_location('sdk_patch_base4',ROOT/'tools/go-java-v1-getty-profile/prepare_candidate.py');base4=importlib.util.module_from_spec(spec);spec.loader.exec_module(base4);old=base4.files()[path]
        parts.append(f'diff --git a/{path} b/{path}\n')
        if not old:parts.append('new file mode 100644\n')
        parts.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),new.decode().splitlines(keepends=True),fromfile='a/'+path if old else '/dev/null',tofile='b/'+path))
        rows.append(dict(path=path,original_sha256=sha(source.read_bytes()) if source.exists() else None,patched_sha256=sha(new)))
    patch=''.join(parts).encode();target=DEST/'0005-java-v1-sdk-session-profile.patch';target.write_bytes(patch)
    requires={path.name:sha(path.read_bytes()) for path in sorted(DEST.glob('000[1-4]-*.patch'))};assert len(requires)==4
    (DEST/'sdk_profile_manifest.json').write_text(json.dumps(dict(go_pin=DEST.name,patch=target.name,patch_sha256=sha(patch),requires=requires,files=rows,
        scope='Explicit real Getty session selection plus SDK reply correlation; default profile disabled, no transaction resend.'),indent=2)+'\n',encoding='utf-8')
    print(str(target))
if __name__=='__main__':run()
