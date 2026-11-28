"""Check restore-gate selection against a sealed actual retryable checksum reply."""
import copy
import json
from pathlib import Path
import shutil
import sys
from inventory_rebuilt import ROOT, need, save, sha
from validate_structured_logs import catalog
import exact_temporal_restart_process as process


def main():
    packet=ROOT/'docs/phase-2/evidence/mysql-exact-temporal-restart/java_20261128_113733_65e8a413'
    digest='8de7113a2d22cfc269474a7b6c3b1692800109d561388cb08673ca4e2537c4d9'
    catalog(packet,digest)
    case=packet/'1.99-boolean-rollback';ready=json.loads((case/'observations/worker-ready.json').read_bytes())
    selected=process.failed_callback(process.live_frames(case/'RM-frames.jsonl'),ready)
    need(selected is not None and selected[1]['index']==12 and selected[1]['body'][2]==1 and selected[1]['body'][-1]==9,'Actual retryable branch failure not selected')
    rows=list(selected);checks=1
    failure=copy.deepcopy(rows);message=b'resource action not confirmed';body=failure[1]['body'];failure[1]['body']=body[:2]+b'\x00'+len(message).to_bytes(2,'big')+message+b'\x00'+body[4:]
    need(process.failed_callback(failure,ready) is not None,'Explicit failed RPC branch outcome not selected');checks+=1
    need(process.failed_callback([rows[0]],ready) is None,'Missing response authorized restore');checks+=1
    for key in ['connection','request_id']:
        bad=copy.deepcopy(rows);bad[1][key]+=100
        need(process.failed_callback(bad,ready) is None,'Uncorrelated reply authorized restore');checks+=1
    bad=copy.deepcopy(rows);bad[1]['body']=bad[1]['body'][:-1]+b'\x08'
    need(process.failed_callback(bad,ready) is None,'Successful callback authorized corruption restore');checks+=1
    for key,value in [('xid','wrong-xid'),('branch_id',ready['branch_id']+1)]:
        wrong=copy.deepcopy(ready);wrong[key]=value
        try:process.failed_callback(rows,wrong)
        except ValueError:checks+=1
        else:raise AssertionError('Wrong branch identity authorized restore')
    bad=copy.deepcopy(rows);bad[1]['monotonic_ns']=bad[0]['monotonic_ns']-1
    try:process.failed_callback(bad,ready)
    except ValueError:checks+=1
    else:raise AssertionError('Reversed wire order accepted')
    bad=copy.deepcopy(rows);_,pos=process.string16(bad[0]['body'],2);body=bytearray(bad[0]['body']);body[pos+8]=1;bad[0]['body']=bytes(body)
    try:process.failed_callback(bad,ready)
    except ValueError:checks+=1
    else:raise AssertionError('Different branch mode accepted')
    commit=copy.deepcopy(rows);commit[0]['kind']=3;commit[0]['body']=b'\x00\x03'+commit[0]['body'][2:];commit[1]['kind']=4;commit[1]['body']=b'\x00\x04'+commit[1]['body'][2:-1]+b'\x06'
    need(process.failed_callback(commit,ready) is not None,'Commit retryable branch failure not selected');checks+=1
    need(checks==11,'Gate check count differs')
    out=ROOT/'docs/phase-2/evidence/mysql-exact-temporal-restart/process_checks_20261128';out.mkdir(exist_ok=True)
    save(out/'result.json',dict(status='RESTORE_GATE_SELECTOR_PASSED',checks=checks,actual_failure_catalog_sha256=digest,process_source_sha256=sha(Path(process.__file__)),argv=sys.argv,exit_code=0))
    shutil.copyfile(Path(__file__),out/Path(__file__).name)
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='catalog.json'])
    print('RESTORE_GATE_SELECTOR_PASSED checks='+str(checks))


if __name__=='__main__':main()
