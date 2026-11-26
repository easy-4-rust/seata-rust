"""Fixture-only two-TLS MySQL bridge; forwards original handshake and authentication bytes."""
import socket
import ssl
import threading
import select
from pathlib import Path

folder = Path("D:/codex-builds/mysql-tls-lifecycle-20261123/certs")
def packet(peer):
    def exact(size):
        parts = bytearray()
        while len(parts) < size:
            part = peer.recv(size - len(parts))
            if not part:
                raise EOFError()
            parts.extend(part)
        return bytes(parts)
    header = exact(4)
    return header + exact(int.from_bytes(header[:3], "little"))

def relay(left, right):
    peers = (left, right)
    outgoing = {left: bytearray(), right: bytearray()}
    for peer in peers:
        peer.setblocking(False)
    while True:
        if not any(peer.pending() for peer in peers):
            select.select(peers, [peer for peer in peers if outgoing[peer]], [], 0.05)
        # Each SSL object is only touched by this one thread, including TLS 1.3 post-handshake I/O.
        for source, target in ((left, right), (right, left)):
            if len(outgoing[target]) < 1048576:
                try:
                    data = source.recv(65536)
                    if not data:
                        return
                    outgoing[target].extend(data)
                except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass
            if outgoing[source]:
                try:
                    sent = source.send(outgoing[source])
                    del outgoing[source][:sent]
                except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
                    pass

def serve(front):
    try:
        with front, socket.create_connection(("127.0.0.1", 43318), timeout=5) as backend:
            front.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            backend.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            backend.settimeout(5)
            front.settimeout(5)
            front.sendall(packet(backend))
            request = packet(front)
            if len(request) != 36 or not int.from_bytes(request[4:8], "little") & 0x800:
                raise ValueError("TLS SSLRequest required")
            backend.sendall(request)
            backend_context = ssl.create_default_context(cafile=str(folder / "ca.pem"))
            with backend_context.wrap_socket(backend, server_hostname="127.0.0.1") as upstream:
                front_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                front_context.minimum_version = ssl.TLSVersion.TLSv1_2
                front_context.load_cert_chain(str(folder / "bridge-active.pem"), str(folder / "server-key.pem"))
                with front_context.wrap_socket(front, server_side=True) as downstream:
                    relay(downstream, upstream)
    except (OSError, EOFError, ssl.SSLError, ValueError) as error:
        print("fixture bridge connection closed: " + type(error).__name__, flush=True)

with socket.socket() as listener:
    listener.bind(("127.0.0.1", 43319))
    listener.listen(32)
    print("owned lifecycle TLS bridge 127.0.0.1:43319 -> owned MySQL relay", flush=True)
    while True:
        peer, _ = listener.accept()
        threading.Thread(target=serve, args=(peer,), daemon=True).start()
