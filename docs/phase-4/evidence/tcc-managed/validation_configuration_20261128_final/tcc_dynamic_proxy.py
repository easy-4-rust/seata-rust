"""Owned TCC observer: original callbacks, or loss of one actual successful Registered response."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import time
from rm_proxy import CallbackRule, ProxyServer, identity


class DynamicRule(CallbackRule):
    def __init__(self,mode,hold_file=None,release_file=None):
        if mode not in ['duplicate','reconnect','report_lost','report_cancel']:raise ValueError('Unknown dynamic TCC fault')
        super().__init__(mode if mode in ['duplicate','reconnect'] else 'basic','RM')
        self.mode=mode;self.reports={};self.hold_file=hold_file;self.release_file=release_file
        if mode=='report_cancel' and (hold_file is None or release_file is None):raise ValueError('Owned report cancellation controls missing')

    def inspect(self,direction,connection,frame):
        event,sends,close=super().inspect(direction,connection,frame)
        key=(connection,event['request_id']);head=struct.unpack('>H',frame[7:9])[0];body=frame[head:]
        if direction=='c2s' and event['kind']==13:
            xid,branch,end=identity(body,2)
            if event['request_type']!=0 or len(body)<end+8 or body[end:end+3]!=b'\x01\x00\x00':raise ValueError('Expected original Registered report with absent resource')
            size=struct.unpack('>i',body[end+3:end+7])[0]
            if size<=0 or end+7+size+1!=len(body) or body[-1]!=1:raise ValueError('Expected bounded original full TCC report')
            app=body[end+7:-1].decode('utf-8')
            with self.lock:
                if key in self.reports or len(self.reports)>=64:raise ValueError('Repeated or over-budget report correlation')
                self.reports[key]=dict(xid=xid,branch_id=str(branch),application_data=app,request_frame_sha256=hashlib.sha256(frame).hexdigest())
        if direction!='s2c' or event['kind']!=14 or self.mode not in ['report_lost','report_cancel']:return event,sends,close
        with self.lock:
            if self.faults:return event,sends,close
            if event['request_type']!=1 or key not in self.reports or body!=b'\x00\x0e\x01\x00':raise ValueError('Report fault requires original matching successful TC response')
            self.faults=1;report=self.reports[key].copy()
        if self.mode=='report_cancel':
            report.update(connection=connection,request_id=event['request_id'],frame_hex=frame.hex(),monotonic_ns=time.perf_counter_ns())
            temporary=self.hold_file.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');temporary.replace(self.hold_file)
            limit=time.monotonic()+20
            while not self.release_file.exists():
                if time.monotonic()>limit:raise TimeoutError('Owned cancellation observation never released original response')
                time.sleep(.01)
        with self.lock:self.closed=1
        event.update(action='drop_and_close',delivery_count=0)
        return event,[],True


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--listen-port',type=int,required=True);parser.add_argument('--upstream-port',type=int,required=True)
    parser.add_argument('--mode',choices=['duplicate','reconnect','report_lost','report_cancel'],required=True);parser.add_argument('--events',type=Path,required=True);parser.add_argument('--stop-file',type=Path,required=True)
    parser.add_argument('--hold-file',type=Path);parser.add_argument('--release-file',type=Path);args=parser.parse_args()
    for path in [args.hold_file,args.release_file]:
        if path is not None and path.resolve().drive.upper() not in ['D:','E:']:raise ValueError('Owned control path must stay on development drives')
    # 父类负责真实帧、会话、清理和有界预算；规则只截留一份原成功响应，不伪造 TC 决定。
    server=ProxyServer(args.listen_port,args.upstream_port,'basic',args.events,args.stop_file,'RM')
    server.rule=DynamicRule(args.mode,args.hold_file,args.release_file);server.serve()
