"""Verify original TCC retry delivery, same-branch authority and physical exactly-once business effects."""
import argparse
import copy
import json
from pathlib import Path
import subprocess
from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog, sources
from validate_xa_managed import load, string, only
from validate_xa_java_faults import frames
from xa_managed_java_faults import guard


def check_case(folder, prefix, mode, direction, alterations=None):
    case = folder/prefix
    alterations = alterations or {}
    proof = alterations.get('proof',load(case/'observations/proof/original-java-tcc-fault.json'))
    commit = direction == 'commit'
    expected = 'Committed' if commit else 'Rollbacked'
    need(proof['mode'] == mode and proof['expected_tc_terminal'] == expected and proof['initial_global_status'] in [None,expected] and proof['branch_type'] == 1 and proof['lock_key'] is None, 'Original TCC direction/type changed')
    need(proof['journal'] == 'Completed' and proof['client_end_requests'] == 1 and proof['finished_inferred_as_success'] is False and proof['status'] == (2 if commit else 3), 'Unknown client result incorrectly completed or original Fence missing')
    need(proof['business'] == ([[1,14]] if commit else []), 'Business proof differs')
    need(load(case/'client-command.json')['exit_code'] == 0 and load(case/'process-cleanup.json')['stopped'], 'Actual client/cleanup failed')
    rm,summary,events = frames(case/'RM-frames.jsonl')
    tm,tm_summary,_ = frames(case/'TM-frames.jsonl')
    rm = alterations.get('rm',rm)
    summary = alterations.get('summary',summary)
    tm = alterations.get('tm',tm)
    wire = [(r['kind'],r['body'],r['direction']) for r in rm]
    need(not any(r['kind'] == 13 for r in rm), 'TCC sent AT phase-one report')
    register = only(wire,11,'c2s')
    xid,pos = string(register,2)
    need(xid == proof['xid'] and register[pos] == 1, 'Original TCC registration changed')
    resource,pos = string(register,pos+1)
    lock,pos = string(register,pos,4)
    app,pos = string(register,pos,4)
    need(resource == proof['resource_id'] == 'tcc-original-action' and lock is None and app == proof['application_data'] and pos == len(register), 'Registration resource/context/optional lock changed')
    need(json.loads(app) == {'actionContext':{'useTCCFence':True,'id':1,'amount':7}}, 'Original action context changed')
    response = only(wire,12,'s2c')
    branch = int.from_bytes(response[4:12],'big',signed=True)
    need(response[2:4] == b'\x01\x00' and str(branch) == proof['branch_id'], 'Original TC branch changed')
    register_row = next(r for r in rm if r['kind'] == 11 and r['direction'] == 'c2s')
    response_row = next(r for r in rm if r['kind'] == 12 and r['direction'] == 's2c')
    need((register_row['connection'],register_row['request_id']) == (response_row['connection'],response_row['request_id']), 'Registration response not original request')
    callbacks = [r for r in rm if r['kind'] == (3 if commit else 5)]
    replies = [r for r in rm if r['kind'] == (4 if commit else 6)]
    need(callbacks and len(replies) == 2, 'Fault callbacks/replies missing')
    for row in callbacks:
        body = row['body'];value,pos = string(body,2)
        need(row['direction'] == 's2c' and value == xid and int.from_bytes(body[pos:pos+8],'big',signed=True) == branch and body[pos+8] == 1, 'Retry changed original TCC identity')
        value,pos = string(body,pos+9);data,pos = string(body,pos,4)
        need(value == resource and data == app and pos == len(body), 'Retry changed original context/resource')
    for row in replies:
        body = row['body'];value,pos = string(body,4)
        need(row['direction'] == 'c2s' and body[2:4] == b'\x01\x00' and value == xid and int.from_bytes(body[pos:pos+8],'big',signed=True) == branch and body[pos+8:] == bytes([5 if commit else 8]), 'Original TCC terminal reply differs')
        need(any((r['connection'],r['request_id']) == (row['connection'],row['request_id']) for r in callbacks), 'Reply not original callback correlation')
    need(summary['mode'] == mode and summary['faults'] == 1, 'Fault not injected exactly once')
    if mode == 'duplicate':
        need(len(callbacks) == 1 and callbacks[0]['action'] == 'duplicate_original' and callbacks[0]['delivery_count'] == 2, 'Original callback not duplicated exactly')
        need([r['action'] for r in replies] == ['hold','release_original'] and [r['delivery_count'] for r in replies] == [0,1], 'Duplicate replies not held/released exactly')
        need(replies[0]['frame_hex'] == replies[1]['frame_hex'] == replies[1]['released_frame_hex'] and summary['released'] == 1 and summary['closed'] == 0, 'Released reply not original bytes')
    else:
        need([r['action'] for r in replies] == ['drop_and_close','forward'] and [r['delivery_count'] for r in replies] == [0,1] and summary['closed'] == 1 and summary['connections'] >= 2, 'Original terminal loss/close/reconnect missing')
        closes = [r for r in events if r['event'] == 'fault_close']
        need(len(closes) == 1, 'Original RM session not actually closed')
        old,new = replies[0]['connection'],replies[1]['connection']
        need(old < new and callbacks[0]['connection'] == old and all(r['body'] == callbacks[0]['body'] and r['action'] == 'forward' and r['delivery_count'] == 1 for r in callbacks), 'TC did not retry original callback unchanged on new RM session')
        correlations = [(r['connection'],r['request_id']) for r in callbacks]
        need(len(correlations) == len(set(correlations)), 'Duplicate callback request correlation')
        # 原 TC 同步结束与后台重试可能重叠：关闭前旧连接上的未答复请求只能保持未决。
        old_callbacks = [r for r in callbacks if r['connection'] == old]
        new_callbacks = [r for r in callbacks if r['connection'] == new]
        need(len(old_callbacks)+len(new_callbacks) == len(callbacks) and all(r['monotonic_ns'] < closes[0]['monotonic_ns'] for r in old_callbacks) and len(new_callbacks) == 1 and new_callbacks[0]['request_id'] == replies[1]['request_id'], 'Unmatched callback not bounded by original closed session')
        need(any(r['kind'] == 103 and r['connection'] == new for r in rm) and any(r['kind'] == 104 and r['connection'] == new and r['body'][2:4] == b'\x01\x00' for r in rm), 'New RM session did not register again')
    need(all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in rm if r not in callbacks+replies), 'Unrelated RM message altered')
    need(tm_summary['mode'] == 'basic' and all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in tm), 'TM decision altered')
    tm_wire = [(r['kind'],r['body'],r['direction']) for r in tm]
    only(tm_wire,1,'c2s')
    begin = only(tm_wire,2,'s2c')
    need(begin[2:4] == b'\x01\x00' and string(begin,4)[0] == xid, 'Original TC global Begin differs')
    need(string(only(tm_wire,7 if commit else 9,'c2s'),2)[0] == xid and not any(r['kind'] in [15,16] for r in tm), 'Client End repeated or status query used as outcome')
    terminal = load(case/'original-tc-terminal.json')
    branch_log = ('Commit' if commit else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)
    global_log = ('Committing global transaction is successfully done, xid = ' if commit else 'Rollback global transaction successfully, xid = ')+xid+'.'
    need(terminal == dict(xid=xid,branch_id=str(branch),direction=direction,original_branch_success_log=branch_log,original_global_success_log=global_log,Finished_used_as_success=False), 'TC terminal observation changed')
    logs = alterations.get('tc_logs',(case/'tc-stdout.log').read_bytes())
    need(branch_log.encode() in logs and global_log.encode() in logs, 'Original TC completion logs missing')
    def fact(name):return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    need(fact('independent-state') == (xid.encode().hex().upper()+'\t'+str(branch)+'\t'+('2' if commit else '3')).encode(), 'Original physical Fence differs')
    need(fact('independent-business') == (b'1\t14' if commit else b'') and fact('independent-tc') == b'0\t0\t0' and not fact('independent-prepared') and fact('native-commands') == b'Query\tXA RECOVER CONVERT XID', 'Physical TCC effect/cleanup differs')
    need(fact('business-dml').splitlines() == [b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(1,7)',b'Execute\t'+(b'UPDATE tcc_managed SET v=14 WHERE id=1' if commit else b'DELETE FROM tcc_managed WHERE id=1')], 'Try or terminal business DML replayed/changed')


def verify(folder,digest):
    catalog(folder,digest);sources(folder)
    result = load(folder/'result.json')
    need(result['Rust_versions'] == ['1.99','1.85'] and result['modes'] == ['duplicate','reconnect'] and result['Java_pin'] == guard.reg.PIN and result['Java_jar_sha256'] == guard.reg.JAR_SHA and result['TCC_full_fault_gate_complete'] is result['whole_plan_complete'] is False, 'Original TC fault scope changed')
    original = subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java'])
    need((folder/'original-DefaultCore.java').read_bytes() == original, 'Terminal source not original pinned TC')
    for version in ['1.99','1.85']:
        for mode in ['duplicate','reconnect']:
            for direction in ['commit','rollback']:check_case(folder,version+'-'+mode+'-'+direction,mode,direction)
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned fixture database remains')


def weakenings(folder):
    rejected = 0
    overlap_cases = 0
    for version in ['1.99','1.85']:
        for mode in ['duplicate','reconnect']:
            for direction in ['commit','rollback']:
                prefix = version+'-'+mode+'-'+direction;case = folder/prefix
                proof = load(case/'observations/proof/original-java-tcc-fault.json')
                changes = []
                for key,value in [('finished_inferred_as_success',True),('client_end_requests',2),('branch_type',2),('journal','Pending')]:
                    changed = copy.deepcopy(proof);changed[key] = value;changes.append({'proof':changed})
                rm,summary,_ = frames(case/'RM-frames.jsonl')
                changed = copy.deepcopy(summary);changed['faults'] = 0;changes.append({'summary':changed})
                changed = copy.deepcopy(rm);next(r for r in changed if r['kind'] == (4 if direction == 'commit' else 6))['delivery_count'] = 1;changes.append({'rm':changed})
                changed = copy.deepcopy(rm);row = next(r for r in changed if r['kind'] == (4 if direction == 'commit' else 6));row['body'] = row['body'][:-1]+b'\x0a';changes.append({'rm':changed})
                changed = [r for r in copy.deepcopy(rm) if r['kind'] != 11];changes.append({'rm':changed})
                changes.extend([{'tc_logs':b'Finished'},{'facts':{'independent-state':b''}},{'facts':{'business-dml':b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(999,999)'}},{'facts':{'independent-tc':b'1\t0\t0'}}])
                if mode == 'reconnect':
                    changed = copy.deepcopy(rm)
                    next(r for r in reversed(changed) if r['kind'] == (3 if direction == 'commit' else 5))['connection'] = 1
                    changes.append({'rm':changed})
                    changes.append({'rm':[r for r in copy.deepcopy(rm) if r['kind'] != 103 or r['connection'] == 1]})
                    old = [r for r in rm if r['kind'] == (3 if direction == 'commit' else 5) and r['connection'] == 1]
                    if len(old) > 1:
                        overlap_cases += 1
                        for field,value in [('monotonic_ns',2**63),('request_id',old[0]['request_id'])]:
                            changed = copy.deepcopy(rm)
                            next(r for r in changed if r['index'] == old[1]['index'])[field] = value
                            changes.append({'rm':changed})
                for alteration in changes:
                    try:check_case(folder,prefix,mode,direction,alteration)
                    except ValueError:rejected += 1
                    else:raise RuntimeError('TCC original TC evidence weakening accepted')
    need(rejected == 104+2*overlap_cases, 'TCC fault weakening matrix incomplete')
    return rejected


if __name__ == '__main__':
    parser=argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('catalog_sha256');parser.add_argument('--weakenings',action='store_true');args=parser.parse_args()
    verify(args.folder,args.catalog_sha256)
    if args.weakenings:print('Rejected TCC callback fault evidence weakenings:',weakenings(args.folder))
    print('Verified eight original Java TC TCC callback duplication/reconnect cases without business replay')
