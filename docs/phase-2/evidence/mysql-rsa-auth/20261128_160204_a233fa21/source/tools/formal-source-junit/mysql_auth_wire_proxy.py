"""Record owned MySQL auth packets; explicitly inject only the selected server public key."""
import hashlib
import json
import socket
import threading
import time


class AuthProxy:
    def __init__(self, folder, listener, upstream, replacement=None):
        self.folder, self.listener, self.upstream = folder, listener, upstream
        self.replacement = replacement
        self.socket = socket.socket(); self.socket.bind(('127.0.0.1', listener)); self.socket.listen()
        self.socket.settimeout(.2); self.active = True; self.lock = threading.Lock()
        self.events = []; self.peers = []; self.threads = []; self.connections = 0

    def record(self, **value):
        with self.lock:
            value.update(sequence=len(self.events), monotonic_ns=time.perf_counter_ns()); self.events.append(value)

    def start(self):
        self.thread = threading.Thread(target=self.accept, daemon=True); self.thread.start()

    def accept(self):
        while self.active:
            try: client, _ = self.socket.accept()
            except socket.timeout: continue
            except OSError: break
            self.connections += 1; n = self.connections
            task = threading.Thread(target=self.run, args=(client, n), daemon=True); self.threads.append(task); task.start()

    @staticmethod
    def receive(peer, size):
        out = bytearray()
        while len(out) < size:
            value = peer.recv(size - len(out))
            if not value: raise EOFError
            out.extend(value)
        return bytes(out)

    def frame(self, peer, direction, connection):
        header = self.receive(peer, 4); size = int.from_bytes(header[:3], 'little')
        if size > 16_777_215: raise ValueError('MySQL packet budget')
        frame = header + self.receive(peer, size)
        self.record(event='packet', direction=direction, connection=connection, frame_hex=frame.hex(), sha256=hashlib.sha256(frame).hexdigest())
        return frame

    def run(self, client, connection):
        server = None
        try:
            server = socket.create_connection(('127.0.0.1', self.upstream), timeout=5)
            client.settimeout(30); server.settimeout(30)
            with self.lock: self.peers.extend([client, server])
            client.sendall(self.frame(server, 'server-to-client', connection))
            first = self.frame(client, 'client-to-server', connection); server.sendall(first)
            tls = len(first) == 36 and int.from_bytes(first[4:8], 'little') & 0x800 != 0
            self.record(event='channel', connection=connection, tls=tls)

            def relay(source, target, direction):
                try:
                    while True:
                        if tls:
                            frame = source.recv(65536)
                            if not frame: break
                            self.record(event='tls_bytes', connection=connection, direction=direction, size=len(frame), sha256=hashlib.sha256(frame).hexdigest())
                        else:
                            frame = self.frame(source, direction, connection)
                            body = frame[4:]
                            if direction == 'server-to-client' and body.startswith(b'\x01-----BEGIN PUBLIC KEY-----') and self.replacement is not None:
                                replacement = b'\x01' + self.replacement
                                changed = len(replacement).to_bytes(3, 'little') + frame[3:4] + replacement
                                self.record(event='key_replacement', connection=connection, original_sha256=hashlib.sha256(frame).hexdigest(), forwarded_hex=changed.hex(), forwarded_sha256=hashlib.sha256(changed).hexdigest())
                                frame = changed
                        target.sendall(frame)
                except (OSError, EOFError): pass
                finally:
                    try: target.shutdown(socket.SHUT_WR)
                    except OSError: pass
            upstream = threading.Thread(target=relay, args=(client, server, 'client-to-server'), daemon=True); upstream.start()
            relay(server, client, 'server-to-client'); upstream.join(timeout=3)
        except (OSError, EOFError) as error:
            self.record(event='connection_end', connection=connection, kind=type(error).__name__)
        finally:
            client.close()
            if server: server.close()

    def close(self):
        self.active = False; self.socket.close(); self.thread.join(timeout=3)
        with self.lock: peers = list(self.peers)
        for peer in peers:
            try: peer.shutdown(socket.SHUT_RDWR)
            except OSError: pass
            peer.close()
        for thread in self.threads: thread.join(timeout=5)
        self.folder.joinpath('mysql-auth-frames.json').write_text(json.dumps(self.events, indent=2), encoding='utf-8')
        self.folder.joinpath('mysql-proxy-result.json').write_text(json.dumps(dict(connections=self.connections, tls_translation=False, public_key_injection=self.replacement is not None)), encoding='utf-8')
