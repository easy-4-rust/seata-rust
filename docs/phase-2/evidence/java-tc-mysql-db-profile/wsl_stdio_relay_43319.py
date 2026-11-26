import socket
import subprocess
import sys
import threading

LINUX = r"""
import os
import socket
import sys
import threading

def upload(peer):
    try:
        while True:
            value = os.read(0, 65536)
            if not value:
                break
            peer.sendall(value)
    except OSError:
        pass
    finally:
        try:
            peer.shutdown(socket.SHUT_WR)
        except OSError:
            pass

with socket.create_connection(('127.0.0.1', 43317), timeout=5) as peer:
    peer.settimeout(None)
    threading.Thread(target=upload, args=(peer,), daemon=True).start()
    while True:
        value = peer.recv(65536)
        if not value:
            break
        sys.stdout.buffer.write(value)
        sys.stdout.buffer.flush()
"""

def forward(client):
    with client, subprocess.Popen(
        ['wsl.exe', '-d', 'Ubuntu-20.04', '--exec', '/usr/bin/python3',
         '-u', '/mnt/d/codex-builds/seata-wsl-mysql-20261120/linux_stdio_relay.py'],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    ) as worker:
        def upload():
            try:
                while value := client.recv(65536):
                    worker.stdin.write(value)
                    worker.stdin.flush()
            except OSError:
                pass
            finally:
                try:
                    worker.stdin.close()
                except OSError:
                    pass
        sender = threading.Thread(target=upload, daemon=True)
        sender.start()
        try:
            while value := worker.stdout.read1(65536):
                client.sendall(value)
        except OSError:
            pass
        finally:
            try:
                client.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            sender.join(timeout=1)
            try:
                worker.wait(timeout=1)
            except subprocess.TimeoutExpired:
                worker.kill()

if __name__ == '__main__':
    from pathlib import Path
    Path('D:/codex-builds/seata-wsl-mysql-20261120/linux_stdio_relay.py').write_text(LINUX, 'utf-8')
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 43319))
        listener.listen(32)
        print('owned MySQL loopback relay 127.0.0.1:43319', flush=True)
        while True:
            client, _ = listener.accept()
            threading.Thread(target=forward, args=(client,), daemon=True).start()

