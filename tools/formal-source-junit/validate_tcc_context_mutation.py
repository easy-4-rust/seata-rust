"""Bind mutable-context fixtures to executed unchanged Java sources and current Rust behavior evidence."""
import argparse
import copy
import json
from pathlib import Path
import subprocess

from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog
from xa_managed_java_faults import guard


def load(path):
    return json.loads(path.read_bytes())


def semantic(rows):
    pairs = [r for r in rows if r['kind'] == 'pair']
    states = {r['name']:r for r in rows if r['kind'] == 'state'}
    need(len(pairs) == 30 and len(states) == 16 and len(rows) == 46, 'Java mutation scope differs')
    lookup = {(r['before'],r['after']):r for r in pairs}
    for key,changed in [(('1','1.0'),True),(('1.0','1.00'),False),(('0.0','-0.0'),True),(('1','null'),False),(('1e10000','2e10000'),False),(('1.23000000000000000000000000001','1.23000000000000000000000000002'),False),(('{"a":1}','{"a":2}'),True),(('[1,2]','[2,1]'),True)]:
        need(lookup[key]['changed'] is changed and lookup[key].get('updated') is (True if changed else None), 'Original Java equality observation changed')
    for name,updated in [('initial',None),('untracked_put',None),('enable',None),('same_put',None),('new_null',True),('same_null',None),('missing_remove',None),('remove_null',True),('clear',True),('clear_empty',None),('replace_map',None),('delay_true',True),('delay_false',True),('reset',None),('add_null',None),('util_delay_object',True)]:
        row = states[name]
        need(row.get('updated') is updated and row['xid'] == 'original-xid' and row['branch_id'] == '9007199254740995', 'Original tracking/identity observation changed')
    need(states['delay_true']['delay_report'] is True and states['delay_false']['delay_report'] is False and states['reset'].get('delay_report') is None, 'Java nullable delay flag changed')


def verify(folder,digest):
    catalog(folder,digest)
    result = load(folder/'result.json')
    need(result['status'] == 'PINNED_JAVA_TCC_CONTEXT_MUTATIONS_EXECUTED' and result['Java_pin'] == guard.reg.PIN and result['Java_jar_sha256'] == guard.reg.JAR_SHA and result['vectors'] == 46 and result['TCC_gate_complete'] is result['whole_plan_complete'] is False, 'Java mutation pin/scope differs')
    for name,path in [('BusinessActionContext.java','integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContext.java'),('BusinessActionContextUtil.java','integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContextUtil.java'),('ActionContextUtil.java','integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/interceptor/ActionContextUtil.java')]:
        original = subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':'+path])
        need((folder/name).read_bytes() == original, 'Reference source was modified')
    need((folder/'MutationOracle.java').read_bytes() == (ROOT/'tools/tcc/java/MutationOracle.java').read_bytes(), 'Executed Java observation wrapper drifted')
    need(all(c['exit_code'] == 0 for c in load(folder/'commands.json')), 'Actual Java compile/run failed')
    rows = [json.loads(line.split(b'\t',1)[1]) for line in (folder/'oracle.stdout').read_bytes().splitlines() if line.startswith(b'MUTATION\t')]
    fixture = ROOT/result['fixture']
    need(load(fixture) == rows and sha(fixture) == result['fixture_sha256'], 'Rust fixture is not actual Java observations')
    origins = (folder/'class-load.log').read_bytes().splitlines()
    for name in ['org.apache.seata.rm.tcc.api.BusinessActionContext','org.apache.seata.rm.tcc.api.BusinessActionContext$TrackedActionContextMap','org.apache.seata.rm.tcc.api.BusinessActionContextUtil','org.apache.seata.integration.tx.api.interceptor.ActionContextUtil']:
        need(any((name+' source:').encode() in line and b'/classes/' in line for line in origins), 'Newly compiled unchanged Java object not actually loaded')
    semantic(rows)
    return rows


def weakenings(rows):
    changed = copy.deepcopy(rows)
    candidates = []
    for index,row in enumerate(rows):
        if row['kind'] == 'pair' and (row['before'],row['after']) in [('1','1.0'),('1.0','1.00'),('0.0','-0.0'),('1','null'),('1e10000','2e10000'),('1.23000000000000000000000000001','1.23000000000000000000000000002'),('{"a":1}','{"a":2}'),('[1,2]','[2,1]')]:
            changed = copy.deepcopy(rows);changed[index]['changed'] = not row['changed'];candidates.append(changed)
        if row['kind'] == 'state':
            changed = copy.deepcopy(rows);changed[index]['updated'] = not bool(row.get('updated'));candidates.append(changed)
    rejected = 0
    for changed in candidates:
        try:semantic(changed)
        except ValueError:rejected += 1
        else:raise RuntimeError('Java context evidence weakening accepted')
    need(rejected == 24, 'Context evidence weakening matrix incomplete')
    return rejected


if __name__ == '__main__':
    parser=argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('catalog_sha256');parser.add_argument('--weakenings',action='store_true');args=parser.parse_args()
    rows = verify(args.folder,args.catalog_sha256)
    if args.weakenings:print('Rejected Java context evidence weakenings:',weakenings(rows))
    print('Verified 46 unchanged Java observations; Rust integration is verified by current workspace evidence')
