"""Independent TCC process/DB recovery proof, physical SQL, native packets and RM journal checks."""
import argparse
import copy
import json
from pathlib import Path
import struct
import zlib

from inventory_rebuilt import need
from mysql_undo_executor_live import IMAGE
from validate_structured_logs import catalog, sources

STAGES = ['try_uncommitted','tried_commit','tried_cancel','try_ok','confirm_uncommitted','cancel_uncommitted','confirm_ok','cancel_ok','suspended_ok']
MODES = ['none','restart','crash']
APP = '{"actionContext":{"useTCCFence":true,"id":81,"amount":7}}'
INSERT = b'Execute\tINSERT INTO tcc_managed(id,v) VALUES(81,7)'
UPDATE = b'Execute\tUPDATE tcc_managed SET v=14 WHERE id=81'
DELETE = b'Execute\tDELETE FROM tcc_managed WHERE id=81'


def load(path):
    return json.loads(path.read_bytes())


def journal(data, xid, branch, commit):
    need(data[:8] == b'SRMJNL01', 'Original RM journal header differs')
    pos = 8
    states = []
    payload = struct.pack('>HH',3 if commit else 5,len(xid)) + xid.encode() + struct.pack('>qBH',branch,1,len('tcc-original-action')) + b'tcc-original-action' + struct.pack('>I',len(APP)) + APP.encode()
    while pos < len(data):
        size = int.from_bytes(data[pos:pos+4],'big');pos += 4
        record = data[pos:pos+size];pos += size
        need(size >= 13 and len(record) == size and zlib.crc32(record[:-4]) == int.from_bytes(record[-4:],'big'), 'RM journal record truncated or CRC changed')
        need(int.from_bytes(record[:8],'big') == len(states)+1 and record[9:-4] == payload, 'Original journal identity/mode/direction/context differs')
        states.append(record[8])
    need(states == [1,3], 'Original Pending was not followed by one durable Completed')


def check_case(folder, version, mode_index, stage_index, alterations=None):
    alterations = alterations or {}
    mode,stage = MODES[mode_index],STAGES[stage_index]
    observations = folder/'observations'/version/('case-'+str(mode_index))
    matrix = load(observations/'proof/tcc-process-crash-matrix.json')
    need(matrix['database_fault'] == mode and matrix['whole_plan_complete'] is False and len(matrix['cases']) == 9, 'TCC process scope changed')
    row = alterations.get('row',matrix['cases'][stage_index])
    prefix = version+'-case-'+str(mode_index)+'-'+stage
    def fact(name):
        return alterations.get('facts',{}).get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    def record(name):
        return alterations.get(name,load(folder/(prefix+'-'+name+'.json')))
    xid,branch = 'tcc:process:'+stage,9400+stage_index
    commit = stage in ['tried_commit','try_ok','confirm_uncommitted','confirm_ok']
    terminal = stage.startswith(('confirm_','cancel_')) or stage == 'suspended_ok'
    cut = None if stage == 'try_uncommitted' else 2 if stage == 'confirm_ok' else 3 if stage == 'cancel_ok' else 4 if stage == 'suspended_ok' else 1
    final = 4 if stage in ['try_uncommitted','suspended_ok'] else 2 if commit else 3
    need(row['stage'] == stage and row['database_fault'] == mode and row['xid'] == xid and row['branch_id'] == str(branch), 'Original TCC branch changed')
    need(type(row['writer_pid']) is int and row['writer_pid'] > 0 and row['writer_pid'] == row['checkpoint_pid'] and row['writer_pid'] != row['recovery_pid'] == matrix['rust_process_id'], 'Writer and recovery are not distinct actual processes')
    need(row['writer_exited'] is True and type(row['writer_exit_code']) is int and row['writer_exit_code'] != 0, 'Owned writer was not forcibly exited')
    before,after = record('writer-before'),record('writer-after')
    need(before == dict(pid=row['writer_pid'],alive=True,exit_code=259) and after == dict(pid=row['writer_pid'],alive=False,exit_code=row['writer_exit_code']), 'Independent retained OS handle did not observe live-to-exited writer')
    receipt = load(observations/'processes'/stage/'child/proof/receipt.json')
    need(receipt['rust_process_id'] == row['writer_pid'] and receipt['xid'] == xid and receipt['branch_id'] == str(branch) and receipt['resource_id'] == 'tcc-original-action' and receipt['application_data'] == APP, 'Actual SDK successful receipt differs')
    need(row['actual_uncommitted_lock'] is stage.endswith('uncommitted'), 'Actual uncommitted InnoDB lock missing')
    if stage.endswith('uncommitted'):
        checkpoint = load(observations/'processes'/stage/'child/proof/uncommitted.json')
        need(checkpoint == dict(stage=stage,rust_process_id=row['writer_pid']), 'Actual held transaction checkpoint differs')
    need(row['fence_at_cut'] == row['fence_after_exit'] == cut and row['final_status'] == final and row['journal'] == 'Completed' and row['old_journal'] == ('Pending' if terminal else 'None') and row['callback_status'] == (5 if commit else 8), 'Crash boundary or original Pending recovery differs')
    need(row['business_at_cut'] == ([[81,7]] if cut == 1 else [[81,14]] if cut == 2 else []) and row['business'] == ([[81,14]] if commit else []), 'Crash changed atomic business/Fence effects')
    uuid = row['uuid_before']
    need(type(uuid) is str and len(uuid) == 36 and uuid == row['uuid_after'], 'Original MySQL instance UUID changed')
    def physical(status):
        value = uuid.encode()
        if status is not None:
            value += b'\n'+(xid.encode().hex().upper()+'\t'+str(branch)+'\t'+str(status)).encode()
        if status in [1,2]:value += b'\n81\t'+(b'7' if status == 1 else b'14')
        return value
    need(fact('database-cut') == fact('database-before') == fact('database-after') == physical(cut) and fact('database-final') == physical(final), 'Independent original UUID/Fence/business differs across writer/database recovery')
    pre = [] if stage == 'suspended_ok' else [INSERT]
    if stage.startswith('confirm_'):pre.append(UPDATE)
    if stage.startswith('cancel_'):pre.append(DELETE)
    post = [UPDATE if commit else DELETE] if cut == 1 else []
    need(fact('business-dml-before').splitlines() == pre and fact('business-dml').splitlines() == post, 'Business DML replayed, skipped or changed across process death')
    if stage.endswith('_ok'):
        request,ok = [bytes(packet) for packet in row['held_packet']]
        need(request == b'\x07\x00\x00\x00\x03COMMIT' and len(ok) >= 11 and int.from_bytes(ok[:3],'little') == len(ok)-4 and ok[3:5] == b'\x01\x00', 'Held packet is not original COMMIT and correlated actual OK')
        need(row['matched_commands'] == {'try_ok':1,'confirm_ok':2,'cancel_ok':3,'suspended_ok':1}[stage], 'Intercepted wrong COMMIT boundary')
    else:
        need(row['held_packet'] is row['matched_commands'] is None, 'Unexpected control packet boundary')
    if mode != 'none':
        stopped = alterations.get('stopped-inspect',load(folder/(prefix+'-stopped-inspect.stdout')))[0]
        need(stopped['Image'] == IMAGE and not stopped['State']['Running'] and stopped['State']['ExitCode'] == (137 if mode == 'crash' else 0), 'Owned database did not actually stop/crash')
        request = load(observations/'processes'/stage/'restart-request.json')
        need(request == dict(stage=stage,mode=mode,writer_exited=True,uuid=uuid), 'Restart did not follow writer exit')
    data = alterations.get('journal',(observations/'processes'/stage/'child/owner/branches.log').read_bytes())
    journal(data,xid,branch,commit)


def verify(folder,digest):
    catalog(folder,digest);sources(folder)
    result = load(folder/'result.json')
    need(result['Rust_versions'] == ['1.99','1.85'] and result['database_fault_modes'] == MODES and result['TCC_full_gate_complete'] is result['whole_plan_complete'] is False, 'Process matrix scope changed')
    need(result['status'] == 'TCC_OWNED_PROCESS_AND_SAME_DATA_DATABASE_RECOVERY_VERIFIED', 'Process run was not successful')
    commands = load(folder/'commands.json')
    final = load(folder/'inspect.stdout')[0]
    create = (folder/'create.stdout').read_bytes().strip().decode()
    need(final['Id'] == create and final['Image'] == IMAGE and final['State']['Running'], 'Final owned container identity changed')
    for version in ['1.99','1.85']:
        for index,mode in enumerate(MODES):
            for stage in STAGES:
                if mode == 'none':continue
                prefix = version+'-case-'+str(index)+'-'+stage
                stopped = load(folder/(prefix+'-stopped-inspect.stdout'))[0]
                need(stopped['Id'] == create and stopped['Mounts'] == final['Mounts'] and stopped['Config']['Labels'] == final['Config']['Labels'], 'Database recovery replaced original container/data directory')
                for suffix,operation in [('start','start'),('kill','kill')] if mode == 'crash' else [('start','start'),('stop','stop')]:
                    selected = [c for c in commands if c['label'] == prefix+'-'+suffix]
                    need(len(selected) == 1 and selected[0]['argv'][:2] == ['docker',operation] and selected[0]['argv'][-1] == create, 'Lifecycle command did not target original owned database')
    need(all(c['exit_code'] == 0 for c in commands if c['label'] != 'confirm-removed'), 'Native process/database command failed')
    for version in ['1.99','1.85']:
        for index,mode in enumerate(MODES):
            need(b'test result: ok. 1 passed;' in (folder/(version+'-case-'+str(index)+'.stdout')).read_bytes(), 'Actual process test summary missing')
            for stage_index in range(9):check_case(folder,version,index,stage_index)
    need(load(folder/'cleanup.json')['owned_container_removed'], 'Owned database fixture remains')


def weakenings(folder):
    count = 0
    for version in ['1.99','1.85']:
        for mode_index,mode in enumerate(MODES):
            matrix = load(folder/'observations'/version/('case-'+str(mode_index))/'proof/tcc-process-crash-matrix.json')
            for stage_index,stage in enumerate(STAGES):
                row = matrix['cases'][stage_index]
                candidates = []
                for key,value in [('writer_exited',False),('checkpoint_pid',1),('uuid_after','changed'),('old_journal','Completed'),('final_status',0)]:
                    changed = copy.deepcopy(row);changed[key] = value;candidates.append(dict(row=changed))
                prefix = version+'-case-'+str(mode_index)+'-'+stage
                for name,value in [('writer-before',False),('writer-after',True)]:
                    changed = load(folder/(prefix+'-'+name+'.json'));changed['alive'] = value;candidates.append({name:changed})
                candidates += [dict(facts={'database-final':b'changed'}),dict(facts={'business-dml':INSERT}),dict(facts={'database-cut':b'changed'})]
                if stage.endswith('uncommitted'):
                    changed = copy.deepcopy(row);changed['actual_uncommitted_lock'] = False;candidates.append(dict(row=changed))
                if stage.endswith('_ok'):
                    for key,value in [('held_packet',[[0],[0]]),('matched_commands',0)]:
                        changed = copy.deepcopy(row);changed[key] = value;candidates.append(dict(row=changed))
                if mode != 'none':
                    changed = load(folder/(prefix+'-stopped-inspect.stdout'));changed[0]['State']['Running'] = True;candidates.append({'stopped-inspect':changed})
                for alteration in candidates:
                    try:check_case(folder,version,mode_index,stage_index,alteration)
                    except ValueError:count += 1
                    else:raise RuntimeError('TCC process evidence weakening accepted')
    need(count == 642, 'Incomplete TCC process weakening matrix')
    return count


if __name__ == '__main__':
    parser = argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('catalog_sha256');parser.add_argument('--weakenings',action='store_true');args = parser.parse_args()
    verify(args.folder,args.catalog_sha256)
    if args.weakenings:print('Rejected TCC process evidence weakenings:',weakenings(args.folder))
    print('Verified 54 formal RM TCC writer crash / same-data MySQL recovery cases; full TCC and whole-plan gates remain open')
