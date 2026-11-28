"""Actual reset authorization controls; preserve their inputs and verdicts."""
import datetime
import json
from pathlib import Path
import time
import uuid

from inventory_rebuilt import ROOT, save, sha, need
from timestamp_restart_proxy import expected_producer_reset


def run():
    stamp = datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8]
    out = ROOT / 'docs/phase-2/evidence/mysql-timestamp-restart' / ('reset_controls_' + stamp)
    out.mkdir(parents=True)
    valid = dict(phase='timestamp-producer-kill', writer_pid=101, supervisor_pid=102, monotonic_ns=time.perf_counter_ns())
    reset = ConnectionResetError('controlled socket reset')
    cases = []
    variants = [('wrong-phase', dict(valid, phase='joint-kill')),
                ('same-process', dict(valid, supervisor_pid=101)),
                ('missing-owner', {k:v for k,v in valid.items() if k != 'supervisor_pid'}),
                ('unknown-key', dict(valid, wildcard=True)),
                ('non-integer-owner', dict(valid, writer_pid='101')),
                ('zero-owner', dict(valid, writer_pid=0)),
                ('expired', dict(valid, monotonic_ns=time.perf_counter_ns() - 11_000_000_000)),
                ('future', dict(valid, monotonic_ns=time.perf_counter_ns() + 11_000_000_000))]
    for name, value in variants:
        path = out / (name + '.json')
        save(path, value)
        need(expected_producer_reset(reset, path) is None, 'Invalid reset permission accepted: ' + name)
        cases.append(dict(case=name, accepted=False))
    path = out / 'valid.json'
    save(path, valid)
    need(expected_producer_reset(reset, path) == sha(path), 'Authorized reset not bound to exact bytes')
    need(expected_producer_reset(ConnectionAbortedError('controlled socket abort'), path) == sha(path), 'Authorized abort not recognized')
    for name, error, authorization in [('other-error', ValueError('not a socket reset'), path),
                                      ('no-permission', reset, None), ('missing-file', reset, out / 'absent.json')]:
        need(expected_producer_reset(error, authorization) is None, 'Unrelated reset exception ignored: ' + name)
        cases.append(dict(case=name, accepted=False))
    save(out / 'result.json', dict(status='EXPLICIT_PRODUCER_RESET_AUTHORIZATION_VERIFIED', positive_cases=2,
                                  negative_cases=cases, rejected=len(cases)))
    source = Path(__file__)
    (out / source.name).write_bytes(source.read_bytes())
    (out / 'timestamp_restart_proxy.py').write_bytes((source.parent / 'timestamp_restart_proxy.py').read_bytes())
    save(out / 'catalog.json', [dict(path=p.relative_to(out).as_posix(), sha256=sha(p))
                               for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    run()
