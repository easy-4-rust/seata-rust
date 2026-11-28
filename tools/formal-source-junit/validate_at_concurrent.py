"""Independent finite AT load verification; no performance SLA is inferred."""
import argparse, copy, json, math, struct, zlib
from pathlib import Path
from inventory_rebuilt import ROOT, need, sha, save
from validate_structured_logs import catalog, sources
from validate_tcc_joint_crash import frames
from validate_xa_managed import string
from xa_managed_java_faults import guard


def load(path):
    return json.loads(path.read_bytes())


def correlated(rows, request, kind):
    result = [r for r in rows if r['kind'] == kind and r['direction'] != request['direction']
              and (r['connection'], r['request_id']) == (request['connection'], request['request_id'])]
    need(len(result) == 1 and result[0]['index'] > request['index'], 'Missing/duplicate correlated response')
    return result[0]


def journal(data, transactions, resource):
    need(data[:8] == b'SRMJNL01', 'Missing durable journal header')
    states = {}; pos = 8; sequence = 0
    identities = {(t['xid'], t['branch_id']): t for t in transactions}
    while pos < len(data):
        need(pos + 4 <= len(data), 'Truncated journal length')
        size = int.from_bytes(data[pos:pos+4], 'big'); pos += 4
        record = data[pos:pos+size]; pos += size; sequence += 1
        need(size >= 13 and len(record) == size and zlib.crc32(record[:-4]) == int.from_bytes(record[-4:], 'big'), 'Journal CRC/size differs')
        need(int.from_bytes(record[:8], 'big') == sequence, 'Journal sequence differs')
        phase = int.from_bytes(record[9:11], 'big'); xid, offset = string(record, 11)
        branch = int.from_bytes(record[offset:offset+8], 'big', signed=True); offset += 8
        branch_type = record[offset]; offset += 1
        actual_resource, offset = string(record, offset); app, offset = string(record, offset, 4)
        key = (xid, branch); need(key in identities, 'Journal acquired unknown branch')
        need(phase == (3 if identities[key]['commit'] else 5) and branch_type == 0 and actual_resource == resource and app is None and offset == len(record)-4, 'Journal original identity changed')
        previous = states.get(key); current = record[8]
        need(current in [1, 2, 3] and (current == 1 if previous is None else current == previous or (previous == 1 and current in [2,3]) or (previous == 2 and current == 1)), 'Illegal journal state transition')
        states[key] = current
    need(pos == len(data) and set(states) == set(identities) and set(states.values()) == {3}, 'Some branch not durably Completed')


def case(folder, version, concurrency, changes=None):
    changes = changes or {}; prefix = version + '-c' + str(concurrency); root = folder/prefix
    proof = changes.get('proof', load(root/'observations/result.json')); txs = proof['transactions']; count = concurrency*8
    need(proof['status'] == 'AT_CONCURRENT_LOAD_VERIFIED' and proof['concurrency'] == concurrency and proof['rounds'] == 8 and proof['tc_remaining'] == [0,0,0] and proof['tm_closed'] is True, 'Load scope or cleanup changed')
    need(len(txs) == count and [t['id'] for t in txs] == list(range(1,count+1)), 'Missing/duplicate load transaction')
    need(len({t['xid'] for t in txs}) == len({t['branch_id'] for t in txs}) == count and all(t['branch_id'] > 0 and t['commit'] == (t['id'] % 2 == 0) for t in txs), 'Transaction identity/direction changed')
    for t in txs:
        need(0 <= t['started_us'] < t['committed_us'] <= t['end_started_us'] < t['finished_us'] <= proof['wall_us'], 'Invalid transaction timing')
        need(t['observed_end_status'] in (['Committed'] if t['commit'] else ['Rollbacked','RollbackRetrying']), 'Non-authoritative end outcome accepted')
    command = load(root/'client-command.json'); need(command['exit_code'] == 0 and len(command['sha256']) == 64 and b'test result: ok. 1 passed;' in (root/'client.stdout').read_bytes(), 'Native load did not execute')
    need(command['argv'] == [command['path'],'original_java_tc_at_concurrent_load','--ignored','--exact','--nocapture'], 'Wrong native executable target')
    classes=(root/'class-load.log').read_bytes()
    expected=('org.apache.seata.server.coordinator.DefaultCore source: jar:nested:/'+(guard.reg.ORACLE/'server/target/seata-server.jar').as_posix()+'/!BOOT-INF/classes/!/').encode()
    need(any(expected in line for line in classes.splitlines()), 'Original coordinator not loaded from pinned jar')
    rm = changes.get('rm', frames(root/'RM-frames.jsonl')[0]); tm = changes.get('tm', frames(root/'TM-frames.jsonl')[0])
    need(all(r['action'] == 'forward' and r['delivery_count'] == 1 for r in rm+tm), 'Native load altered/replayed a frame')
    reg = [r for r in rm if r['kind'] == 103 and r['direction'] == 'c2s']; need(len(reg) == 1, 'Unexpected RM reconnect/registration')
    correlated(rm, reg[0], 104)
    pos = 2; fields = []
    for _ in range(4): value,pos = string(reg[0]['body'],pos); fields.append(value)
    resource,pos = string(reg[0]['body'],pos,4)
    need(fields == ['1.1.0','phase0-auth','default_tx_group','phase0-local-only'] and pos == len(reg[0]['body']) and resource.startswith('jdbc:mysql://127.0.0.1:'), 'Original RM identity differs')
    begins = [r for r in tm if r['kind'] == 1 and r['direction'] == 'c2s']
    registers = [r for r in rm if r['kind'] == 11 and r['direction'] == 'c2s']
    reports = [r for r in rm if r['kind'] == 13 and r['direction'] == 'c2s']
    ends = [r for r in tm if r['kind'] in [7,9] and r['direction'] == 'c2s']
    need(len(begins) == len(registers) == len(reports) == len(ends) == count and not any(r['kind'] in [15,16] for r in tm), 'Begin/branch/report/end replayed or missing')
    known_xids={t['xid'] for t in txs}
    need(all(string(r['body'],2)[0] in known_xids for r in rm if r['direction']=='s2c' and r['kind'] in [3,5]), 'Callback for an unrelated transaction')
    begin_map = {}
    for row in begins:
        response = correlated(tm,row,2); xid,pos = string(response['body'],4)
        need(response['body'][2:4] == b'\x01\x00' and response['body'][pos:] == b'\x00\x00' and xid not in begin_map, 'Original Begin not successful/unique')
        begin_map[xid] = row
    end_map = {}
    for row in ends:
        xid,pos = string(row['body'],2); need(row['body'][pos:] == b'\x00\x00' and xid not in end_map, 'End repeated/changed')
        end_map[xid] = row; correlated(tm,row,8 if row['kind'] == 7 else 10)
    for t in txs:
        xid,branch = t['xid'],t['branch_id']; need(xid in begin_map and xid in end_map and end_map[xid]['kind'] == (7 if t['commit'] else 9), 'End reversed/original XID absent')
        end_reply=correlated(tm,end_map[xid],8 if t['commit'] else 10)['body']
        status={'Committed':9,'Rollbacked':11,'RollbackRetrying':5}[t['observed_end_status']]
        need(end_reply[2:]==b'\x01\x00'+bytes([status]), 'Observed outcome not bound to actual original TC response')
        selected = [r for r in registers if string(r['body'],2)[0] == xid]; need(len(selected) == 1, 'Registration switched XID')
        row = selected[0]; body = row['body']; _,pos = string(body,2); need(body[pos] == 0, 'Branch not AT'); actual,pos = string(body,pos+1); lock,pos = string(body,pos,4); app,pos = string(body,pos,4)
        need(actual == resource and app is None and pos == len(body) and sorted(lock.split(';')) == sorted('undo_work:'+str(t['id']+i) for i in [0,10000,20000]), 'Wrong lock domain/keys/application data')
        response = correlated(rm,row,12); need(response['body'][2:4] == b'\x01\x00' and int.from_bytes(response['body'][4:12],'big',signed=True) == branch, 'Original receipt changed')
        selected = [r for r in reports if string(r['body'],2)[0] == xid]; need(len(selected) == 1, 'Report wrong identity')
        report = selected[0]; _,pos = string(report['body'],2)
        need(int.from_bytes(report['body'][pos:pos+8],'big',signed=True) == branch and report['body'][pos+8] == 2, 'Report is not PhaseOneDone')
        actual,pos=string(report['body'],pos+9); app,pos=string(report['body'],pos,4)
        need(actual==resource and app is None and report['body'][pos:]==b'\x00', 'Report resource/application data/AT kind differs')
        ack = correlated(rm,report,14); need(ack['body'] == b'\x00\x0e\x01\x00', 'Report ACK not success')
        callbacks = [r for r in rm if r['direction'] == 's2c' and r['kind'] in [3,5] and string(r['body'],2)[0] == xid]; need(0 < len(callbacks) <= 64, 'Missing/unbounded original callback')
        terminal = False
        for callback in callbacks:
            body = callback['body']; _,pos = string(body,2)
            need(callback['kind'] == (3 if t['commit'] else 5) and int.from_bytes(body[pos:pos+8],'big',signed=True) == branch and body[pos+8] == 0, 'Callback original identity/direction differs')
            actual,pos = string(body,pos+9); app,pos = string(body,pos,4); need(actual == resource and app is None and pos == len(body), 'Callback resource/app differs')
            reply = correlated(rm,callback,4 if t['commit'] else 6); body = reply['body']; actual,pos = string(body,4)
            need(body[2:4] == b'\x01\x00' and actual == xid and int.from_bytes(body[pos:pos+8],'big',signed=True) == branch and pos+9 == len(body), 'Callback response not original/correlated')
            need(body[-1] in ([5,6] if t['commit'] else [8,9]), 'Callback status not terminal/retryable')
            terminal |= body[-1] == (5 if t['commit'] else 8)
        need(terminal, 'Original branch never received success')
        logs=changes.get('logs',(root/'tc-stdout.log').read_bytes())
        need((('Commit' if t['commit'] else 'Rollback')+' branch transaction successfully, xid = '+xid+' branchId = '+str(branch)).encode() in logs, 'Original TC did not confirm branch success')
    for number in range(8):
        checkpoint = changes.get('round-'+str(number),load(root/('observations/round-'+str(number)+'.json')))
        expected = txs[number*concurrency:(number+1)*concurrency]
        need(checkpoint['round'] == number and checkpoint['concurrency'] == concurrency and len(checkpoint['identities']) == len(checkpoint['snapshots']) == concurrency, 'Actual overlap checkpoint missing')
        for item, snapshot, tx in zip(checkpoint['identities'],checkpoint['snapshots'],expected):
            need(item == {k:tx[k] for k in ['id','xid','branch_id','commit','started_us','committed_us']} and snapshot == dict(xid=tx['xid'],branch_id=tx['branch_id'],counts=[1,1,1,3,1,1]), 'Simultaneous Undo/SHA/witness/report/locks not observed')
            need(tx['committed_us'] <= checkpoint['observed_us'] <= tx['end_started_us'], 'Barrier released before independent observation')
        need(max(begin_map[t['xid']]['monotonic_ns'] for t in expected) < min(end_map[t['xid']]['monotonic_ns'] for t in expected), 'No native global transaction overlap')
    def fact(name): return changes.get(name,(folder/(prefix+'-'+name+'.stdout')).read_bytes().strip().replace(b'\r',b''))
    expected_business = sorted((t['id']+offset,value) for t in txs for offset,value in ([(0,'updated'),(10000,'inserted')] if t['commit'] else [(0,'before'),(20000,'delete_before')]))
    need(fact('independent-business') == '\n'.join(str(i)+'\t'+v for i,v in expected_business).encode(), 'Physical business data differs')
    expected_undo = sorted((t['branch_id'],t['xid'].encode().hex().upper()+'\t'+str(t['branch_id'])+'\t1') for t in txs if not t['commit'])
    need(fact('independent-undo') == '\n'.join(v for _,v in expected_undo).encode(), 'Undo cleanup/fence differs')
    states = fact('independent-state').splitlines(); need(len(states) == count, 'Physical terminal records missing')
    for row,t in zip(states, sorted(txs,key=lambda t:t['branch_id'])):
        columns = row.decode().split('\t'); need(columns[:7] == [t['xid'].encode().hex().upper(),str(t['branch_id']),*(['1','0','1','1'] if t['commit'] else ['2','1','0','0']),'1'] and len(columns) == 8 and len(columns[7]) == 64, 'Physical original branch/witness/digest differs')
    need(fact('independent-tc') == b'0\t0\t0', 'TC resources leaked')
    journal(changes.get('journal',(root/'observations/branches.log').read_bytes()),txs,resource)
    need(load(root/'process-cleanup.json')['stopped'] is True, 'Owned processes still running')
    # 测量包含有意屏障、观察与清理；仅报告该有限实验，不推断生产吞吐。
    durations = sorted(t['finished_us']-t['started_us'] for t in txs)
    return dict(Rust=version,concurrency=concurrency,transactions=count,wall_us=proof['wall_us'],
                transactions_per_second=count*1_000_000/proof['wall_us'],
                latency_us={str(q):durations[max(0,math.ceil(len(durations)*q)-1)] for q in [.5,.95,.99]},
                rollback_retrying=sum(t['observed_end_status']=='RollbackRetrying' for t in txs))


def verify(folder, digest):
    catalog(folder,digest); sources(folder); result = load(folder/'result.json')
    need(result['status'] == 'ORIGINAL_JAVA_TC_AT_CONCURRENT_LOAD_VERIFIED' and result['Java_pin'] == guard.reg.PIN and result['Java_jar_sha256'] == guard.reg.JAR_SHA and result['rounds_per_level'] == 8 and result['whole_plan_complete'] is False and result['production_capacity_certified'] is False, 'Incorrect scope/pinned Java')
    need(result['Rust_versions'] and set(result['Rust_versions']) <= {'1.99','1.85'} and result['concurrency_levels'] and set(result['concurrency_levels']) <= {1,4,8}, 'Invalid toolchain/load scope')
    need(result['actual_global_transactions'] == 8*sum(result['concurrency_levels'])*len(result['Rust_versions']), 'Actual load count differs')
    need(all(r['exit_code'] == (1 if r['label']=='confirm-removed' else 0) for r in load(folder/'commands.json')) and load(folder/'cleanup.json')['owned_container_removed'] is True, 'Command failed/owned container remains')
    need((folder/'java-pin.stdout').read_text().strip() == guard.reg.PIN, 'Wrong original Java checkout')
    binding=load(folder/'auth-binding.json')
    need(set(binding['runtime']) == {'META-INF/services/org.apache.seata.core.rpc.RegisterCheckAuthHandler','org/apache/seata/server/auth/CallbackOwnershipAuthHandler.class','org/apache/seata/server/auth/Phase0CredentialCheckAuthHandler.class'}, 'Overlay replaced original coordinator')
    need(all(sha(Path(name)) == digest for name,digest in binding['sources'].items()), 'Authentication fixture source drift')
    final=load(folder/'inspect-stopped.stdout'); initial=load(folder/'inspect.stdout'); created=(folder/'create.stdout').read_text().strip()
    need(len(final)==len(initial)==1 and initial[0]['Id']==final[0]['Id']==created and final[0]['Image']==initial[0]['Image']==guard.IMAGE and not final[0]['State']['Running'] and final[0]['State']['ExitCode']==0, 'Owned container identity/image/stop differs')
    create=next(r['argv'] for r in load(folder/'commands.json') if r['label']=='create'); label=create[create.index('--label')+1]; key,value=label.split('=',1)
    mount=create[create.index('--mount')+1]; source=mount.split('source=',1)[1].split(',target=',1)[0]
    need(final[0]['Config']['Labels'][key]==value and any(m['Destination']=='/var/lib/mysql' and m['Source'].replace('\\','/').casefold()==source.casefold() for m in final[0]['Mounts']), 'Owned data mount/owner differs')
    return [case(folder,v,c) for v in result['Rust_versions'] for c in result['concurrency_levels']]


def weakenings(folder):
    result=load(folder/'result.json'); rejected=0
    for v in result['Rust_versions']:
        for c in result['concurrency_levels']:
            root=folder/(v+'-c'+str(c)); proof=load(root/'observations/result.json'); variants=[]
            for key,value in [('tm_closed',False),('transactions',proof['transactions'][:-1]),('wall_us',0),('concurrency',99)]:
                changed=copy.deepcopy(proof);changed[key]=value;variants.append(dict(proof=changed))
            changed=copy.deepcopy(proof);changed['transactions'][0]['commit']=True;variants.append(dict(proof=changed))
            changed=copy.deepcopy(proof);changed['transactions'][0]['end_started_us']=changed['transactions'][0]['committed_us'];variants.append(dict(proof=changed))
            for key in ['independent-business','independent-state','independent-undo','independent-tc','journal','logs']:variants.append({key:b'changed'})
            for key,value in [('counts',[1,1,1,2,1,1]),('xid','other')]:
                changed=load(root/'observations/round-0.json');changed['snapshots'][0][key]=value;variants.append({'round-0':changed})
            for target,kind in [('tm',1),('tm',7 if c>1 else 9),('rm',11),('rm',13),('rm',5)]:
                changed=frames(root/('TM-frames.jsonl' if target=='tm' else 'RM-frames.jsonl'))[0];changed=[r for r in changed if r['kind']!=kind];variants.append({target:changed})
            changed=frames(root/'RM-frames.jsonl')[0];next(r for r in changed if r['kind']==14)['body']=b'\x00\x0e\x00\x00';variants.append(dict(rm=changed))
            for variant in variants:
                try:case(folder,v,c,variant)
                except ValueError:rejected+=1
                else:raise RuntimeError('AT concurrent evidence weakening accepted')
    return rejected


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('sha256');parser.add_argument('--weakenings',action='store_true');parser.add_argument('--report',type=Path);args=parser.parse_args()
    measurements=verify(args.folder,args.sha256); rejected=weakenings(args.folder) if args.weakenings else 0
    report=dict(status='AT_CONCURRENT_EVIDENCE_VERIFIED',measurements=measurements,evidence_weakenings_rejected=rejected,production_capacity_certified=False,whole_plan_complete=False)
    if args.report:save(args.report,report)
    print(json.dumps(report,ensure_ascii=False))
