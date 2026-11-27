"""Bind actual TCC transactions to original V1 frames, physical Fence rows and independent DML logs."""
import argparse
import copy
import json
import re
from pathlib import Path
from inventory_rebuilt import ROOT, need, sha
from validate_structured_logs import catalog, sources
from validate_xa_managed import raw_frames, string, only
from xa_managed_java_faults import guard


def load(path):
    return json.loads(path.read_bytes())


def mysql_case(folder, version, index, alterations=None):
    prefix = version+'-case-'+str(index)
    names = ['normal','failed-actions','early-cancel','identity','ack-loss','held-try','registration-boundaries','cancelled-try']
    proof = load(folder/'observations'/version/('case-'+str(index))/'proof'/(names[index]+'.json'))
    facts = {name:(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b'') for name in ['business','fence','undo','business-dml']}
    if alterations:
        proof = alterations.get('proof',proof)
        facts.update(alterations.get('facts',{}))
    need(b'test result: ok. 1 passed;' in (folder/(prefix+'.stdout')).read_bytes(), 'Actual TCC case did not execute')
    need(type(proof['rust_process_id']) is int and proof['rust_process_id'] > 0 and facts['undo'] == b'0', 'TCC changed Java Undo or has no process observation')
    def fence(rows):
        return b'\n'.join((xid.encode().hex().upper()+'\t'+str(branch)+'\t'+str(status)).encode() for xid,branch,status in sorted(rows))
    insert = lambda i: 'INSERT INTO tcc_managed(id,v) VALUES('+str(i)+',7)'
    update = lambda i: 'UPDATE tcc_managed SET v=14 WHERE id='+str(i)
    delete = lambda i: 'DELETE FROM tcc_managed WHERE id='+str(i)
    if index == 0:
        rows = proof['branches']
        need(len(rows) == 2 and [r['status'] for r in rows] == [2,3] and all(r['duplicate_try_rejected'] and r['journal'] == 'Completed' for r in rows), 'Normal Fence lifecycle changed')
        need(facts['fence'] == fence([(r['xid'],r['branch_id'],r['status']) for r in rows]) and proof['business'] == [[1,14]], 'Original normal Fence differs')
        business, dml = b'1\t14', [insert(1),update(1),insert(2),delete(2)]
    elif index == 1:
        need(len(proof['cases']) == 3 and [r['case'] for r in proof['cases']] == [0,1,2] and all(r['no_partial_effect'] for r in proof['cases']) and proof['fresh_manager_confirmed'], 'Failed business or caught DML error committed partial effects')
        row = proof['cases'][2]
        need(facts['fence'] == fence([(row['xid'],row['branch_id'],2)]), 'Failed Confirm did not recover original Fence')
        business, dml = b'21\t14', [insert(21),insert(21),insert(21),update(21),update(21)]
    elif index == 2:
        need(proof['status'] == 4 and proof['late_try_rejected'] and proof['original_receipt_preserved'] and proof['manual_missing_is_unknown'], 'Empty Cancel did not preserve callback authority/late Try denial')
        need(facts['fence'] == fence([(proof['xid'],proof['branch_id'],4)]), 'Original empty Cancel Fence differs')
        business, dml = b'', []
    elif index == 3:
        need(proof['row_corruptions_rejected'] == 5 and proof['physical_changes_rejected'] == 4 and proof['restored_original_completed'], 'Physical/identity counterexamples missing')
        need(facts['fence'] == fence([(proof['xid'],proof['branch_id'],2)]), 'Restored original Fence differs')
        business, dml = b'31\t14', [insert(31),update(31)]
    elif index == 4:
        rows = proof['cases']
        need([r['stage'] for r in rows] == ['try','confirm','cancel'], 'COMMIT OK loss matrix incomplete')
        for row,status,commands in zip(rows,[1,2,3],[1,2,3],strict=True):
            request, response = bytes(row['held_request']), bytes(row['held_original_ok'])
            need(int.from_bytes(request[:3],'little') == len(request)-4 and request[3:] == b'\x00\x03COMMIT', 'Captured request not actual COM_QUERY COMMIT')
            need(int.from_bytes(response[:3],'little') == len(response)-4 and response[3:5] == b'\x01\x00', 'Original successful COMMIT OK missing')
            need(row['database_status_before_recovery'] == status and row['old_journal'] == ('NoCallbackYet' if row['stage'] == 'try' else 'Pending') and row['journal'] == 'Completed' and row['fresh_manager_and_source'] and row['proxy_commit_commands'] == commands, 'Unknown commit/journal/fresh recovery lifecycle changed')
        row = rows[-1]
        need(facts['fence'] == fence([(row['xid'],row['branch_id'],3)]), 'Original lost Cancel terminal differs')
        business, dml = b'', [insert(51),update(51),insert(52),update(52),insert(53),delete(53)]
    elif index == 5:
        need(proof['actual_innodb_wait_observed'] and proof['uncommitted_fence_and_business_invisible'] and proof['status'] == 3 and proof['late_try_rejected'], 'Try/Cancel race lacks actual database lock/invisibility evidence')
        need(facts['fence'] == fence([(proof['xid'],proof['branch_id'],3)]), 'Original held Try rollback differs')
        business, dml = b'', [insert(41),delete(41)]
    elif index == 6:
        rows = proof['cases']
        need([r['mode'] for r in rows] == ['rejected','lost','cancelled'] and all(r['branch_id'] is None and r['successful_receipt_not_synthesized'] and r['invalid_inputs_rejected_before_registration'] == 6 and r['original_register_requests'] == 1 and r['try_business_rows'] == 0 for r in rows), 'Registration outcome guessed a successful receipt/branch or started Try')
        need([r['fence_rows'] for r in rows] == [0,0,1] and [r['status'] for r in rows] == [None,None,4] and all(r['callback_branch_id'] is None for r in rows[:2]), 'Registration without authoritative callback minted Fence')
        row = rows[-1]
        need(row['callback_branch_id'] == '9202' and facts['fence'] == fence([(row['xid'],row['callback_branch_id'],4)]), 'Late registration reply did not preserve original callback Fence')
        business, dml = b'', []
    else:
        rows = proof['cases']
        need([r['stage'] for r in rows] == ['before_commit','after_commit'] and [r['status'] for r in rows] == [4,3] and all(r['known_receipt_retained'] and r['old_call_cancelled'] and r['late_try_rejected'] for r in rows), 'Cancelled Try did not preserve original receipt or distinct commit outcomes')
        need(rows[0]['actual_innodb_wait_observed'] and rows[1]['committed_TRIED_and_business_observed'], 'Cancellation boundary not physically observed')
        request,response = bytes(rows[1]['held_request']),bytes(rows[1]['held_original_ok'])
        need(int.from_bytes(request[:3],'little') == len(request)-4 and request[3:] == b'\x00\x03COMMIT' and int.from_bytes(response[:3],'little') == len(response)-4 and response[3:5] == b'\x01\x00', 'Cancelled Try did not capture actual original COMMIT OK')
        row = rows[-1]
        need(facts['fence'] == fence([(row['xid'],row['branch_id'],3)]), 'Original cancelled committed Try terminal differs')
        business, dml = b'', [insert(61),insert(62),delete(62)]
    need(facts['business'] == business, 'Independent TCC business differs')
    expected_business = [[int(v) for v in line.split(b'\t')] for line in business.splitlines()]
    need(proof['business'] == expected_business, 'Reported TCC business differs from physical result')
    actual = []
    for line in facts['business-dml'].splitlines():
        command,sql = line.split(b'\t',1)
        need(command in [b'Execute',b'Query'], 'Unexpected independent DML command family')
        actual.append(sql.decode())
    need(actual == dml, 'Business DML replayed, missing or changed')


def java_case(folder, version, direction, alterations=None):
    prefix = version+'-'+direction
    case = folder/prefix
    proof = load(case/'observations/proof/original-java-tcc.json')
    alterations = alterations or {}
    proof = alterations.get('proof',proof)
    commit = direction == 'commit'
    need(proof['global_status'] == ('Committed' if commit else 'Rollbacked') and proof['journal'] == 'Completed' and proof['status'] == (2 if commit else 3) and proof['branch_type'] == 1 and proof['finished_inferred_as_success'] is False, 'Original TC did not explicitly complete TCC direction')
    need(load(case/'client-command.json')['exit_code'] == 0 and load(case/'process-cleanup.json')['stopped'], 'Original Java client/cleanup failed')
    rm = alterations.get('rm',raw_frames(case/'RM-frames.jsonl'))
    need(not any(code == 13 for code,_,_ in rm), 'TCC emitted AT phase-one report')
    register = only(rm,11,'c2s')
    xid,pos = string(register,2)
    need(xid == proof['xid'] and register[pos] == 1, 'Original TCC registration changed')
    resource,pos = string(register,pos+1)
    lock,pos = string(register,pos,4)
    app,pos = string(register,pos,4)
    need(resource == proof['resource_id'] and lock is None and app == proof['application_data'] and pos == len(register), 'Original action/optional lock/application changed')
    need(json.loads(app) == {'actionContext':{'useTCCFence':True,'id':1,'amount':7}}, 'Original action context changed')
    response = only(rm,12,'s2c')
    branch = int.from_bytes(response[4:12],'big',signed=True)
    need(response[2:4] == b'\x01\x00' and str(branch) == proof['branch_id'], 'Original TC branch ID differs')
    callback = only(rm,3 if commit else 5,'s2c')
    value,pos = string(callback,2)
    need(value == xid and int.from_bytes(callback[pos:pos+8],'big',signed=True) == branch and callback[pos+8] == 1, 'Original callback identity/type differs')
    value,pos = string(callback,pos+9)
    data,pos = string(callback,pos,4)
    need(value == resource and data == app and pos == len(callback), 'TC callback context/resource differs')
    reply = only(rm,4 if commit else 6,'c2s')
    value,pos = string(reply,4)
    need(reply[2:4] == b'\x01\x00' and value == xid and int.from_bytes(reply[pos:pos+8],'big',signed=True) == branch and reply[pos+8:] == bytes([5 if commit else 8]), 'Original TCC terminal response differs')
    tm = alterations.get('tm',raw_frames(case/'TM-frames.jsonl'))
    only(tm,1,'c2s')
    need(string(only(tm,7 if commit else 9,'c2s'),2)[0] == xid, 'Original global End identity/count changed')
    end = only(tm,8 if commit else 10,'s2c')
    need(end[2:4] == b'\x01\x00' and end[-1] == (9 if commit else 11), 'Original TC explicit global status differs')
    def fact(name): return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    need(fact('independent-state') == (xid.encode().hex().upper()+'\t'+str(branch)+'\t'+('2' if commit else '3')).encode(), 'Original physical TCC Fence differs')
    need(fact('independent-business') == (b'1\t14' if commit else b'') and fact('independent-tc') == b'0\t0\t0' and not fact('independent-prepared') and fact('native-commands') == b'Query\tXA RECOVER CONVERT XID', 'TCC business/TC cleanup/native XA differs')
    dml = fact('business-dml').splitlines()
    need(dml == [b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(1,7)',b'Execute\t'+(b'UPDATE tcc_managed SET v=14 WHERE id=1' if commit else b'DELETE FROM tcc_managed WHERE id=1')], 'Original TC business DML replayed/changed')


def verify(mysql, java, regressions):
    for folder,digest in [mysql,java,*regressions]:
        catalog(folder,digest)
        sources(folder)
    for pair in [mysql,java]:
        result = load(pair[0]/'result.json')
        need(result['Rust_versions'] == ['1.99','1.85'] and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False and load(pair[0]/'cleanup.json')['owned_container_removed'], 'TCC runtime scope/cleanup changed')
    need(load(java[0]/'result.json')['Java_pin'] == guard.reg.PIN and load(java[0]/'result.json')['Java_jar_sha256'] == guard.reg.JAR_SHA, 'Original Java pin/artifact differs')
    for version in ['1.99','1.85']:
        for index in range(8): mysql_case(mysql[0],version,index)
        for direction in ['commit','rollback']: java_case(java[0],version,direction)
    for version,(folder,_) in zip(['1.99','1.85'],regressions,strict=True):
        result = load(folder/'result.json')
        need(result['Rust'] == version and result['passed'] == 801 and result['ignored'] == 392 and result['successful_test_harness_summaries'] == 179 and result['strict_tcc_all_targets_clippy'] and result['strict_xa_mysql_all_targets_clippy'], 'Full workspace regression differs')
        need(all(r['exit_code'] == 0 for r in load(folder/'commands.json')), 'Actual workspace/Clippy command failed')
        summaries = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',(folder/'workspace.stdout').read_bytes())
        need(len(summaries) == 179 and sum(int(r[0]) for r in summaries) == 801 and not any(int(r[1]) for r in summaries) and sum(int(r[2]) for r in summaries) == 392, 'Raw workspace totals differ')


def weakenings(folder):
    rejected = 0
    for version in ['1.99','1.85']:
        for index,name in enumerate(['normal','failed-actions','early-cancel','identity','ack-loss','held-try','registration-boundaries','cancelled-try']):
            proof = load(folder/'observations'/version/('case-'+str(index))/'proof'/(name+'.json'))
            candidates = [{'facts':{'undo':b'1'}},{'facts':{'business':b'999\t999'}},{'facts':{'fence':b''}},{'facts':{'business-dml':b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(999,999)'}}]
            changed = copy.deepcopy(proof)
            if index == 0: changed['branches'][0]['duplicate_try_rejected'] = False
            elif index == 1: changed['cases'][1]['no_partial_effect'] = False
            elif index == 2: changed['manual_missing_is_unknown'] = False
            elif index == 3: changed['row_corruptions_rejected'] = 0
            elif index == 4: changed['cases'][1]['old_journal'] = 'Completed'
            elif index == 5: changed['actual_innodb_wait_observed'] = False
            elif index == 6: changed['cases'][1]['branch_id'] = '9201'
            else: changed['cases'][0]['status'] = 3
            candidates.append({'proof':changed})
            if index == 4:
                for key,value in [('held_request',[0,0,0,0]),('held_original_ok',[0,0,0,1]),('database_status_before_recovery',0),('fresh_manager_and_source',False),('proxy_commit_commands',99)]:
                    changed = copy.deepcopy(proof);changed['cases'][0][key] = value;candidates.append({'proof':changed})
            for alteration in candidates:
                try: mysql_case(folder,version,index,alteration)
                except ValueError: rejected += 1
                else: raise RuntimeError('TCC semantic weakening accepted')
    need(rejected == 90, 'TCC semantic weakening matrix incomplete')
    return rejected


def java_weakenings(folder):
    rejected = 0
    for version in ['1.99','1.85']:
        for direction in ['commit','rollback']:
            case = folder/(version+'-'+direction)
            proof = load(case/'observations/proof/original-java-tcc.json')
            candidates = []
            for key,value in [('global_status','Finished'),('branch_type',2),('application_data','{}'),('branch_id','1'),('status',0),('journal','Pending'),('finished_inferred_as_success',True)]:
                changed = copy.deepcopy(proof);changed[key] = value;candidates.append({'proof':changed})
            for key,value in [('independent-business',b'999\t999'),('independent-state',b''),('independent-tc',b'1\t0\t0'),('native-commands',b'Query\tXA START'),('business-dml',b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(999,999)'),('independent-prepared',b'9752\tchanged')]:
                candidates.append({'facts':{key:value}})
            frames = raw_frames(case/'RM-frames.jsonl')
            for variant in ['type','context']:
                changed = []
                for kind,body,route in frames:
                    if kind == (3 if direction == 'commit' else 5) and route == 's2c':
                        if variant == 'type':
                            _,pos = string(body,2);raw = bytearray(body);raw[pos+8] = 2;body = bytes(raw)
                        else:
                            body = body.replace(b'"id":1',b'"id":2')
                    changed.append((kind,body,route))
                candidates.append({'rm':changed})
            changed = []
            for kind,body,route in raw_frames(case/'TM-frames.jsonl'):
                if kind == (8 if direction == 'commit' else 10) and route == 's2c': body = body[:-1]+b'\x00'
                changed.append((kind,body,route))
            candidates.append({'tm':changed})
            need(len(candidates) == 16, 'Original TC weakening matrix incomplete')
            for alteration in candidates:
                try: java_case(folder,version,direction,alteration)
                except ValueError: rejected += 1
                else: raise RuntimeError('Original TC semantic weakening accepted')
    need(rejected == 64, 'Original TC weakening count differs')
    return rejected


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ['mysql','java','rust199','rust185']:
        parser.add_argument('--'+name,type=Path,required=True);parser.add_argument('--'+name+'-sha',required=True)
    parser.add_argument('--weakenings',action='store_true')
    args = parser.parse_args()
    pairs = [(getattr(args,name),getattr(args,name+'_sha')) for name in ['mysql','java','rust199','rust185']]
    verify(pairs[0],pairs[1],pairs[2:])
    if args.weakenings: print('Rejected TCC evidence weakenings:',weakenings(pairs[0][0])+java_weakenings(pairs[1][0]))
    print('Verified sixteen MySQL TCC cases, four original Java global transactions and dual workspace evidence; full TCC gate remains open')
