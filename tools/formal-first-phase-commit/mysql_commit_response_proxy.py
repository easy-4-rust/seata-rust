"""Forward an owned MySQL endpoint and hold the selected armed COMMIT OK packet."""

import argparse
import socket
import threading
import time
from pathlib import Path


def packet(peer):
    header = receive(peer, 4)
    size = int.from_bytes(header[:3], "little")
    return header + receive(peer, size)


def receive(peer, size):
    data = bytearray()
    while len(data) < size:
        chunk = peer.recv(size - len(data))
        if not chunk:
            raise EOFError
        data.extend(chunk)
    return bytes(data)


def proxy(client, upstream_port, arm_file, ok_file, commits, skip_commits, hold_before_commit):
    blocked = threading.Event()
    with client, socket.create_connection(("127.0.0.1", upstream_port), timeout=5) as server:
        client.settimeout(None)
        server.settimeout(None)

        def upstream():
            try:
                while True:
                    frame = packet(client)
                    body = frame[4:]
                    candidate = (
                        arm_file.exists()
                        and body[:1] == b"\x03"
                        and body[1:].strip().upper() == b"COMMIT"
                    )
                    intercept = False
                    if candidate:
                        with commits[0]:
                            commits[1][0] += 1
                            intercept = commits[1][0] == skip_commits + 1
                    if intercept and hold_before_commit:
                        ok_file.write_text(
                            "original MySQL COMMIT packet received; not forwarded\n",
                            encoding="utf-8",
                        )
                        time.sleep(2)
                        server.close()
                        return
                    if intercept:
                        blocked.set()
                    server.sendall(frame)
            except (EOFError, OSError):
                try:
                    server.shutdown(socket.SHUT_WR)
                except OSError:
                    pass

        thread = threading.Thread(target=upstream, daemon=True)
        thread.start()
        try:
            while True:
                frame = packet(server)
                if blocked.is_set():
                    if frame[4:5] != b"\x00":
                        raise AssertionError("server did not confirm COMMIT")
                    ok_file.write_text("original MySQL COMMIT returned OK; reply held\n", encoding="utf-8")
                    time.sleep(5)
                    break
                client.sendall(frame)
        except (EOFError, OSError):
            pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen", type=int, required=True)
    parser.add_argument("--upstream", type=int, required=True)
    parser.add_argument("--arm-file", type=Path, required=True)
    parser.add_argument("--ok-file", type=Path, required=True)
    parser.add_argument("--skip-commits", type=int, default=0)
    parser.add_argument("--hold-before-commit", action="store_true")
    args = parser.parse_args()
    if not 0 < args.listen < 65536 or not 0 < args.upstream < 65536:
        parser.error("valid loopback ports required")
    if not 0 <= args.skip_commits <= 8:
        parser.error("bounded skipped COMMIT count required")
    for path in (args.arm_file, args.ok_file):
        if path.exists():
            parser.error(f"stale fault marker: {path}")
        if path.drive.upper() != "D:":
            parser.error("fault markers must remain on D drive")
    with socket.socket() as listener:
        commits = (threading.Lock(), [0])
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", args.listen))
        listener.listen(16)
        print(f"owned MySQL COMMIT proxy 127.0.0.1:{args.listen}", flush=True)
        while True:
            client, _ = listener.accept()
            threading.Thread(
                target=proxy,
                args=(client, args.upstream, args.arm_file, args.ok_file, commits, args.skip_commits, args.hold_before_commit),
                daemon=True,
            ).start()


if __name__ == "__main__":
    main()
