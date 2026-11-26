"""Owned loopback MySQL packet observer; one COM_QUERY COMMIT fault, no TLS or credential capture."""
import hashlib
import socket
import threading
import time

class UndoRollbackCommitObserver:
    def __init__(self, upstream, mode):
        if mode not in ('drop_ok','block_commit','delay_ok','observe'): raise ValueError('unknown fault')
        self.upstream=upstream;self.mode=mode;self.lock=threading.Lock();self.events=[];self.claimed=False;self.stopping=False;self.sockets=[];self.threads=[]
        self.listener=socket.socket();self.listener.bind(('127.0.0.1',0));self.listener.listen();self.listener.settimeout(.2);self.port=self.listener.getsockname()[1]
        self.thread=threading.Thread(target=self.accept,daemon=True);self.thread.start()
    def event(self,connection,kind,**fields):
        with self.lock:self.events.append(dict(connection=connection,kind=kind,qpc_ns=time.perf_counter_ns(),**fields))
    @staticmethod
    def packet(sock):
        def read(n):
            value=b''
            while len(value)<n:
                part=sock.recv(n-len(value))
                if not part: raise EOFError()
                value+=part
            return value
        header=read(4);size=int.from_bytes(header[:3],'little')
        if size>2_000_000: raise ValueError('observer fixture packet budget exceeded')
        return header+read(size)
    @staticmethod
    def close(*sockets):
        for sock in sockets:
            try:sock.shutdown(socket.SHUT_RDWR)
            except OSError:pass
            sock.close()
    def accept(self):
        while not self.stopping:
            try:client,_=self.listener.accept()
            except socket.timeout:continue
            except OSError:break
            server=socket.create_connection(('127.0.0.1',self.upstream));self.sockets.extend((client,server));connection=len(self.sockets)//2
            state={'prepare':None,'statements':{},'begin':False,'rollback':False,'commit':False,'commit_fault':False,'greeting':True,'terminal_candidate':False,'dml_result':None};mutex=threading.Lock()
            for direction in ('client','server'):
                t=threading.Thread(target=self.relay,args=(client,server,connection,state,mutex,direction),daemon=True);self.threads.append(t);t.start()
    def relay(self,client,server,connection,state,mutex,direction):
        source,target=(client,server) if direction=='client' else (server,client)
        try:
            while True:
                wire=self.packet(source);payload=wire[4:];seq=wire[3]
                if direction=='client' and seq==0 and payload:
                    command=payload[0]
                    # 覆盖所有客户端命令；CHANGE_USER等认证内容仅留原header/opcode及整包摘要。
                    self.event(connection,'client_command',opcode=command,wire_prefix_hex=wire[:5].hex(),wire_sha256=hashlib.sha256(wire).hexdigest(),wire_hex=wire.hex() if command in (1,3,14,22,23,25,31) else None)
                    if command in (3,22):
                        sql=payload[1:].decode('utf-8',errors='strict')
                        self.event(connection,'query' if command==3 else 'prepare',sql=sql,wire_hex=wire.hex())
                        with mutex:
                            if command==22:state['prepare']=sql
                        if command==3 and sql.strip().upper()=='BEGIN':
                            self.event(connection,'begin_received',wire_hex=wire.hex())
                            with mutex:
                                state['begin']=True;state['terminal_candidate']=False;target.sendall(wire);self.event(connection,'begin_forwarded',wire_hex=wire.hex())
                            continue
                        if command==3 and sql.strip().upper()=='ROLLBACK':
                            self.event(connection,'rollback_received',wire_hex=wire.hex())
                            with mutex:
                                state['rollback']=True;target.sendall(wire);self.event(connection,'rollback_forwarded',wire_hex=wire.hex())
                            continue
                        if command==3 and sql.strip().upper()=='COMMIT':
                            with self.lock:
                                fault=not self.claimed and state['terminal_candidate'] and self.mode!='observe'
                                if fault:self.claimed=True
                            self.event(connection,'commit_received',fault=fault,wire_hex=wire.hex())
                            if fault and self.mode=='block_commit':
                                self.event(connection,'commit_blocked');self.close(client,server);return
                            with mutex:
                                state['commit']=True;state['commit_fault']=fault
                                target.sendall(wire);self.event(connection,'commit_forwarded',wire_hex=wire.hex())
                            continue
                    elif command==23:
                        statement=int.from_bytes(payload[1:5],'little')
                        with mutex:
                            sql=state['statements'].get(statement)
                            if sql and sql.startswith(('DELETE ','UPDATE ','INSERT ','CREATE TEMPORARY TABLE ','DROP TEMPORARY TABLE ')):state['dml_result']=sql
                            if sql and sql.startswith('INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,application_data,lock_key,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(2,?,?,?,?,?,1,?,0,0,now(6),now(6))'):state['terminal_candidate']=True
                        self.event(connection,'execute',statement=statement,sql=sql,wire_hex=wire.hex())
                if direction=='server':
                    with mutex:
                        dml=state['dml_result'];state['dml_result']=None
                    if dml is not None:
                        self.event(connection,'dml_reply',sql=dml,wire_hex=wire.hex());target.sendall(wire);self.event(connection,'dml_reply_forwarded',sql=dml,wire_hex=wire.hex());continue
                    with mutex:
                        greeting=state['greeting'];state['greeting']=False
                        rollback=state['rollback'];state['rollback']=False;prepare=state['prepare'];commit=state['commit'];commit_fault=state['commit_fault'];begin=state['begin'];state['begin']=False
                        if prepare is not None:
                            state['prepare']=None
                            if payload and payload[0]==255:self.event(connection,'prepare_error',sql=prepare,wire_hex=wire.hex())
                            if payload and payload[0]==0:
                                state['statements'][int.from_bytes(payload[1:5],'little')]=prepare
                                self.event(connection,'prepare_ok',sql=prepare,wire_hex=wire.hex())
                        if commit:state['commit']=False
                    if greeting:
                        end=payload.index(0,1);server_id=int.from_bytes(payload[end+1:end+5],'little');self.event(connection,'greeting',mysql_connection_id=server_id,wire_hex=wire.hex())
                    if begin:
                        self.event(connection,'begin_reply',wire_hex=wire.hex())
                        if not payload or payload[0]!=0:raise ValueError('BEGIN did not return original OK')
                        target.sendall(wire);self.event(connection,'begin_reply_forwarded',wire_hex=wire.hex());continue
                    if rollback:
                        self.event(connection,'rollback_reply',wire_hex=wire.hex())
                        if not payload or payload[0]!=0:raise ValueError('ROLLBACK did not return original OK')
                        target.sendall(wire);self.event(connection,'rollback_reply_forwarded',wire_hex=wire.hex());continue
                    if commit:
                        self.event(connection,'commit_reply',ok=bool(payload and payload[0]==0),wire_hex=wire.hex(),payload_hex=payload.hex(),sha256=hashlib.sha256(payload).hexdigest())
                        if not payload or payload[0]!=0:raise ValueError('fault target did not return actual OK')
                        if not commit_fault:
                            target.sendall(wire);self.event(connection,'commit_reply_forwarded',wire_hex=wire.hex());continue
                        if self.mode=='delay_ok':
                            self.event(connection,'commit_reply_held');time.sleep(2.2)
                            try:target.sendall(wire);self.event(connection,'late_reply_sent')
                            except OSError:self.event(connection,'late_reply_peer_closed')
                        else:self.event(connection,'commit_reply_dropped')
                        self.close(client,server);return
                target.sendall(wire)
        except (OSError,EOFError):pass
        except Exception as error:self.event(connection,'observer_error',error=type(error).__name__)
        finally:self.close(client,server)
    def stop(self):
        self.stopping=True;self.close(self.listener);self.thread.join(timeout=2)
        self.close(*self.sockets)
        for thread in self.threads:thread.join(timeout=2)
        if any(t.is_alive() for t in self.threads) or self.thread.is_alive():raise ValueError('observer threads remain')
        return dict(mode=self.mode,claimed=self.claimed,events=self.events)
