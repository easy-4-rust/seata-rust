"""Hold original TC callbacks before delivery so both original owners can be killed."""
import argparse,json,struct,time
from pathlib import Path
from rm_proxy import CallbackRule,ProxyServer,identity

class JointCrashRule(CallbackRule):
    def __init__(self,cut_file):
        super().__init__('basic','RM');self.mode='joint_crash';self.cut_file=cut_file;self.target=None;self.blocked=0
    def complete(self,connections):return self.faults==1 and self.blocked>0 and self.target is not None and self.closed==0 and self.released==0
    def inspect(self,direction,connection,frame):
        event,sends,close=super().inspect(direction,connection,frame)
        if direction=='s2c' and event['request_type']==0 and event['kind'] in [3,5]:
            head=int.from_bytes(frame[7:9],'big');body=frame[head:];xid,branch,_=identity(body,2)
            with self.lock:
                if self.target is not None and body!=self.target:raise ValueError('Joint-crash held callback changed identity/parameters')
                self.target=body;self.blocked+=1;self.faults=1
                if self.blocked==1:
                    value=dict(xid=xid,branch_id=str(branch),connection=connection,request_id=event['request_id'],kind=event['kind'],frame_hex=frame.hex(),monotonic_ns=time.perf_counter_ns())
                    temporary=self.cut_file.with_suffix('.tmp');temporary.write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8');temporary.replace(self.cut_file)
            event.update(action='hold_for_joint_crash',delivery_count=0);return event,[],False
        if direction=='c2s' and event['kind'] in [4,6]:raise ValueError('Undelivered callback produced a response')
        return event,sends,close

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--listen-port',type=int,required=True);p.add_argument('--upstream-port',type=int,required=True);p.add_argument('--events',type=Path,required=True);p.add_argument('--stop-file',type=Path,required=True);p.add_argument('--cut-file',type=Path,required=True);p.add_argument('--expected-peer-exit-file',type=Path,required=True);a=p.parse_args()
    if a.cut_file.resolve().drive.upper() not in ['D:','E:']:raise ValueError('Control file escaped development drives')
    server=ProxyServer(a.listen_port,a.upstream_port,'basic',a.events,a.stop_file,'RM',a.expected_peer_exit_file);server.rule=JointCrashRule(a.cut_file)
    server.sink.emit(dict(event='joint_crash_rule_installed',mode='joint_crash'));server.serve()
