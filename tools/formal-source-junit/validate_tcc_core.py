"""Verify actual original Java observations, source counterexamples and executed workspace evidence."""
import argparse
import json
from pathlib import Path
import re
import subprocess
from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog, sources
from xa_managed_java_faults import guard


def load(path):
    return json.loads(path.read_bytes())


def verify(java, mutations, regressions):
    for folder, digest in [java, mutations, *regressions]:
        catalog(folder, digest)
    java = java[0]
    result = load(java/'result.json')
    need(result['Java_pin'] == guard.reg.PIN and result['Java_jar_sha256'] == guard.reg.JAR_SHA and result['vectors'] == 21 and result['TCC_gate_complete'] is result['whole_plan_complete'] is False, 'TCC Java scope changed')
    for name, source in [
        ('BusinessActionContext.java','integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContext.java'),
        ('BusinessActionContextUtil.java','integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContextUtil.java'),
        ('ActionContextUtil.java','integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/interceptor/ActionContextUtil.java'),
        ('CommonFenceConstant.java','integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/fence/constant/CommonFenceConstant.java'),
        ('SpringFenceHandler.java','spring/seata-spring/src/main/java/org/apache/seata/rm/fence/SpringFenceHandler.java'),
    ]:
        original = subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':'+source])
        need((java/name).read_bytes() == original, 'Original TCC reference source changed')
    fixture = ROOT/result['fixture']
    raw_rows = [line for line in (java/'oracle.stdout').read_bytes().splitlines() if line.startswith((b'statuses\t',b'java_blank\t')) or b'\taccepted\t' in line or b'\trejected\t' in line]
    need(len(raw_rows) == 23 and fixture.read_bytes() == b'\n'.join(raw_rows)+b'\n' and sha(fixture) == result['fixture_sha256'], 'Rust fixture not actual original Java output')
    need(raw_rows[0] == b'statuses\t1\t2\t3\t4' and len(raw_rows[1].split(b'\t')) == 26, 'Java status/blank observation changed')
    need(all(row['exit_code'] == 0 for row in load(java/'commands.json')), 'Original Java command failed')
    need((java/'ContextOracle.java').read_bytes() == (ROOT/'tools/tcc/java/ContextOracle.java').read_bytes(), 'Final Java wrapper differs')
    origins = (java/'class-load.log').read_bytes().splitlines()
    for name in ['BusinessActionContext','BusinessActionContextUtil']:
        need(any(('org.apache.seata.rm.tcc.api.'+name+' source:').encode() in line and b'/classes/' in line for line in origins), 'Original compiled context not loaded')
    mutations = mutations[0]
    result = load(mutations/'result.json')
    need(result['actual_behavioral_failures'] == 48 and result['variants_per_version'] == 24 and result['live_source_mutated'] is False and result['TCC_gate_complete'] is result['whole_plan_complete'] is False, 'Mutation scope changed')
    for row in load(mutations/'source-inputs.json'):
        need(sha(ROOT/row['path']) == row['sha256'], 'TCC mutation input drift')
    records = load(mutations/'records.json')
    need(len(records) == 48 and len({(r['Rust'],r['mutation']) for r in records}) == 48 and {r['Rust'] for r in records} == {'1.99','1.85'}, 'TCC mutation matrix incomplete')
    for version in ['1.99','1.85']:
        need({r['mutation'] for r in records if r['Rust'] == version} == {'byte_budget','depth_budget','floating_point','string_flag','late_try','terminal_direction','unicode_blank','tracking_off','null_put_not_dirty','add_null_overwrites','remove_null_not_dirty','empty_clear_dirty','set_map_dirty','mutation_budget_ignored','float_zero_collapsed','object_values_ignored','untracked_add_not_dirty','util_delay_ignored','util_same_value_reports','util_batch_null_overwrites','util_clear_before_ack','util_composite_not_string','util_partial_batch_updates','util_noop_clears_dirty'}, 'Required faulty production variant missing')
    for row in records:
        prefix = row['Rust']+'-'+row['mutation']
        need(sha(mutations/(prefix+'.rs')) == row['modified_source_sha256'] and row['exit_code'] == 101, 'Executed faulty source differs')
        stdout = (mutations/(prefix+'.stdout')).read_bytes()
        need(b'test result: FAILED. 0 passed; 1 failed;' in stdout and row['behavioral_test'].encode() in stdout, 'Compiler failure or unexecuted counterexample counted as behavior')
        need(row['argv'][-2:] == ['--','--exact'], 'Counterexample did not select exact test')
    for version, (folder, _) in zip(['1.99','1.85'], regressions, strict=True):
        sources(folder)
        result = load(folder/'result.json')
        need(result['Rust'] == version and result['passed'] == 801 and result['ignored'] == 395 and result['successful_test_harness_summaries'] == 179 and result['strict_tcc_all_targets_clippy'] and result['strict_xa_mysql_all_targets_clippy'], 'Full TCC workspace regression differs')
        need(all(row['exit_code'] == 0 for row in load(folder/'commands.json')), 'Actual workspace/Clippy command failed')
        summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (folder/'workspace.stdout').read_bytes())
        need(len(summaries) == 179 and sum(int(r[0]) for r in summaries) == 801 and not any(int(r[1]) for r in summaries) and sum(int(r[2]) for r in summaries) == 395, 'Raw workspace summaries differ')
        stderr = (folder/'workspace.stderr').read_bytes()
        need(b'tests\\context_and_fence.rs' in stderr and b'tests\\java_context.rs' in stderr and b'tests\\context_util.rs' in stderr, 'New TCC test targets not executed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ['java','mutations','rust199','rust185']:
        parser.add_argument('--'+name, type=Path, required=True)
        parser.add_argument('--'+name+'-sha', required=True)
    args = parser.parse_args()
    pairs = [(getattr(args,name),getattr(args,name+'_sha')) for name in ['java','mutations','rust199','rust185']]
    verify(pairs[0],pairs[1],pairs[2:])
    print('Verified original Java TCC context, 48 behavioral source counterexamples and dual workspace evidence; full TCC gate remains open')
