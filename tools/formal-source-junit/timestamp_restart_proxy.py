"""Reuse the raw observer with one explicitly authorized producer-exit reset."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'formal-rm-java'))
import rm_proxy


def expected_producer_reset(error, authorization):
    if not isinstance(error, (ConnectionResetError, ConnectionAbortedError)) or authorization is None:
        return None
    try:
        raw = Path(authorization).read_bytes()
        value = json.loads(raw)
    except (OSError, ValueError):
        return None
    if set(value) != {'phase', 'writer_pid', 'supervisor_pid', 'monotonic_ns'}:
        return None
    if value['phase'] != 'timestamp-producer-kill':
        return None
    if any(type(value[k]) is not int or value[k] <= 0 for k in ['writer_pid', 'supervisor_pid', 'monotonic_ns']):
        return None
    if value['writer_pid'] == value['supervisor_pid']:
        return None
    if not 0 <= time.perf_counter_ns() - value['monotonic_ns'] <= 10_000_000_000:
        return None
    return hashlib.sha256(raw).hexdigest()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--listen-port', type=int, required=True)
    parser.add_argument('--upstream-port', type=int, required=True)
    parser.add_argument('--mode', choices=['basic'], required=True)
    parser.add_argument('--events', type=Path, required=True)
    parser.add_argument('--stop-file', type=Path, required=True)
    parser.add_argument('--role', choices=['RM', 'TM'], required=True)
    parser.add_argument('--expected-peer-exit-file', type=Path)
    args = parser.parse_args()
    rm_proxy.expected_peer_reset = expected_producer_reset
    rm_proxy.ProxyServer(args.listen_port, args.upstream_port, args.mode, args.events,
                         args.stop_file, args.role, args.expected_peer_exit_file).serve()
