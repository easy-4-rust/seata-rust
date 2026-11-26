"""Bounded passthrough observer for the longer DB checkpoint run; reuses the existing exact-frame reader."""
import argparse
import importlib.util
import json
from pathlib import Path
import socket
import struct
import threading
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('coverage_existing_proxy',HERE.parent/'formal-rm-java/rm_proxy.py')
original=importlib.util.module_from_spec(spec);spec.loader.exec_module(original)

class ReportLossRule(original.CallbackRule):
    def __init__(self):
        super().__init__('basic','RM')
        self.mode='report_loss';self.reports=set()

    def inspect(self,direction,connection,frame):
        event,sends,close=super().inspect(direction,connection,frame)
        key=(connection,event['request_id'])
        if direction=='c2s' and event['request_type']==0 and event['kind']==13:
            if self.reports:raise ValueError('Failure report was resent')
            self.reports.add(key)
        if direction=='s2c' and event['request_type']==1 and event['kind']==14:
            head=struct.unpack('>H',frame[7:9])[0]
            if key not in self.reports or frame[head+2:head+4]!=b'\x01\x00' or self.faults:
                raise ValueError('Unexpected original failure report response')
            self.faults=1;self.closed=1;event.update(action='drop_original_report_reply',delivery_count=0)
            return event,[],True
        return event,sends,close

class TmEndLossRule(original.CallbackRule):
    def __init__(self,rollback=False):
        super().__init__('basic','TM')
        self.mode='tm_rollback_loss' if rollback else 'tm_end_loss'
        self.request_kind=9 if rollback else 7
        self.response_kind=10 if rollback else 8
        self.reports=set()

    def inspect(self,direction,connection,frame):
        event,sends,close=super().inspect(direction,connection,frame)
        key=(connection,event['request_id'])
        if direction=='c2s' and event['request_type']==0 and event['kind']==self.request_kind:
            if self.reports:raise ValueError('Durable TM end was resent')
            self.reports.add(key)
        if direction=='s2c' and event['request_type']==1 and event['kind']==self.response_kind:
            head=struct.unpack('>H',frame[7:9])[0]
            if key not in self.reports or frame[head+2:head+4]!=b'\x01\x00' or self.faults:
                raise ValueError('Unexpected original TM commit response')
            self.faults=1;self.closed=1;event.update(action='drop_original_tm_end_reply',delivery_count=0)
            return event,[],True
        return event,sends,close

class CoverageProxy(original.ProxyServer):
    def __init__(self,listen,upstream,events,stop_file,role,mode='basic',supervised_tm=False):
        super().__init__(listen,upstream,'basic',events,stop_file,role)
        self.cross_process = mode in ('cross_process', 'cross_process_race')
        self.cross_process_race = mode == 'cross_process_race'
        self.supervised_tm = supervised_tm
        if supervised_tm and role != 'TM':raise ValueError('Supervised exit only applies to TM')
        if mode=='report_loss':
            if role!='RM':raise ValueError('Report loss only applies to RM')
            self.rule=ReportLossRule()
            self.sink.emit(dict(event='fault_rule',role=role,mode=mode,kind=14))
        elif mode in ('tm_end_loss','tm_rollback_loss'):
            if role!='TM':raise ValueError('TM end loss only applies to TM')
            self.rule=TmEndLossRule(mode=='tm_rollback_loss')
            self.sink.emit(dict(event='fault_rule',role=role,mode=mode,kind=self.rule.response_kind))
        elif self.cross_process and role!='RM':raise ValueError('Cross-process reset only applies to RM')
        elif mode!='basic' and not self.cross_process:raise ValueError('Unknown coverage mode')

    def serve(self):
        start=time.monotonic();connections=0
        try:
            while not self.stop_file.exists():
                if time.monotonic()-start>300:raise TimeoutError('Coverage observer budget expired')
                try:client,_=self.listener.accept()
                except socket.timeout:continue
                connections+=1
                if connections>8:client.close();raise ValueError('Coverage connection budget exhausted')
                thread=threading.Thread(target=self.session,args=(client,connections));thread.start();self.sessions.append(thread)
        except Exception as error:
            self.errors.append(str(error));self.sink.emit(dict(event='error',message=str(error)))
        finally:
            self.listener.close()
            with self.lock:
                for stream in self.sockets:
                    try:stream.shutdown(socket.SHUT_RDWR)
                    except OSError:pass
            for thread in self.sessions:thread.join(timeout=4)
            if any(thread.is_alive() for thread in self.sessions):self.errors.append('Coverage session not joined')
            expected=(self.rule.faults==1 and self.rule.closed==1 and len(self.rule.reports)==1
                      if self.rule.mode in ('report_loss','tm_end_loss','tm_rollback_loss') else self.rule.faults==0)
            observed = [json.loads(line) for line in Path(self.sink.stream.name).read_text().splitlines()]
            resets = [row for row in observed if row.get('event')=='error' and row.get('connection')==1
                      and row.get('direction')=='c2s' and '[WinError 10054]' in row.get('message','')]
            expected_connections = 2 <= connections <= 4 if self.cross_process_race or self.supervised_tm else connections == 2
            expected_reset = (self.cross_process or self.supervised_tm) and len(resets)==1 and len(self.errors)==1 and expected_connections
            succeeded=(not self.errors or expected_reset) and expected and not self.rule.held and bool(self.rule.roles)
            self.sink.emit(dict(event='summary',role=self.rule.role,succeeded=succeeded,mode='cross_process_race' if self.cross_process_race else 'cross_process' if self.cross_process else self.rule.mode,connections=connections,faults=self.rule.faults,expected_resets=int(expected_reset),errors=self.errors))
            self.sink.close()
        if not succeeded:raise RuntimeError('Coverage observer incomplete')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--listen-port',type=int,required=True);parser.add_argument('--upstream-port',type=int,required=True)
    parser.add_argument('--events',type=Path,required=True);parser.add_argument('--stop-file',type=Path,required=True);parser.add_argument('--role',choices=['RM','TM'],required=True)
    parser.add_argument('--mode',choices=['basic','report_loss','tm_end_loss','tm_rollback_loss','cross_process','cross_process_race'],default='basic')
    parser.add_argument('--supervised-tm',action='store_true')
    args=parser.parse_args();CoverageProxy(args.listen_port,args.upstream_port,args.events,args.stop_file,args.role,args.mode,args.supervised_tm).serve()
