"""Observe owned Rust children in Windows before allowing crash or TC End."""
import hashlib
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import subprocess
import time
from types import SimpleNamespace

from inventory_rebuilt import need, save, sha


def live_frames(path):
    data=path.read_bytes()
    complete=data.splitlines() if data.endswith(b'\n') else data.splitlines()[:-1]
    rows=[]
    for line in complete:
        row=json.loads(line)
        if row.get('event')!='frame':continue
        raw=bytes.fromhex(row['frame_hex']);head=int.from_bytes(raw[7:9],'big')
        need(raw[:3]==b'\xda\xda\x01' and int.from_bytes(raw[3:7],'big')==len(raw) and 16<=head<=len(raw),'Malformed live callback frame')
        need(row['action']=='forward' and row['delivery_count']==1,'Live callback frame changed')
        row['body']=raw[head:];rows.append(row)
    return rows


def string16(body,pos):
    need(pos+2<=len(body),'Truncated callback string length')
    width=int.from_bytes(body[pos:pos+2],'big',signed=True);pos+=2
    need(width>=0 and pos+width<=len(body),'Truncated callback string')
    return (body[pos:pos+width].decode('utf-8') if width else None),pos+width


def failed_callback(rows,ready):
    for callback in rows:
        if callback['direction']!='s2c' or callback['kind'] not in [3,5]:continue
        replies=[r for r in rows if r['direction']=='c2s' and r['kind']==callback['kind']+1 and r['request_id']==callback['request_id'] and r['connection']==callback['connection']]
        for reply in replies:
            body=reply['body'];need(body[2] in [0,1],'Unknown callback result')
            if body[2]==0:
                message,pos=string16(body,3);need(message=='resource action not confirmed','Unexpected failed callback text')
            else:pos=3
            need(body[pos]==0,'Callback exception code changed');pos+=1
            xid,pos=string16(body,pos);need(pos+9==len(body),'Callback identity length differs')
            branch=int.from_bytes(body[pos:pos+8],'big',signed=True);status=body[pos+8]
            if status!=(6 if callback['kind']==3 else 9):continue
            need(xid==ready['xid'] and branch==ready['branch_id'],'Failed callback identity differs')
            requested,at=string16(callback['body'],2)
            need(requested==xid and int.from_bytes(callback['body'][at:at+8],'big',signed=True)==branch and callback['body'][at+8]==0,'Failed callback is not the same original AT branch')
            need(reply['monotonic_ns']>callback['monotonic_ns'],'Callback response preceded request')
            return callback,reply
    return None


def run_client(argv, cwd, env, observations, case_out):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel.GetExitCodeProcess.restype = wintypes.BOOL
    kernel.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel.TerminateProcess.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handles = {}
    records = {}
    stdout = case_out / 'client.stdout'
    stderr = case_out / 'client.stderr'
    process = None
    try:
        with stdout.open('wb') as output, stderr.open('wb') as errors:
            process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=output, stderr=errors,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            deadline = time.monotonic() + 240
            while process.poll() is None:
                need(time.monotonic() < deadline, 'Owned native restart supervisor deadline')
                for role, permission in [('producer', 'allow-producer-kill.json'), ('worker', 'allow-worker-use.json')]:
                    born = observations / (role + '-spawn.json')
                    if role not in records and born.is_file():
                        try:
                            claimed = json.loads(born.read_bytes())
                        except (ValueError, OSError):
                            continue
                        child_pid = int(claimed['pid'])
                        need(claimed['parent_pid'] == process.pid and child_pid > 0, 'Owned child parent claim differs')
                        command = 'Get-CimInstance Win32_Process -Filter "ProcessId=' + str(child_pid) + '" | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine,CreationDate | ConvertTo-Json -Compress'
                        actual = subprocess.run(['powershell.exe', '-NoProfile', '-Command', command],
                            cwd=cwd, env=env, capture_output=True, timeout=15,
                            creationflags=subprocess.CREATE_NO_WINDOW)
                        need(actual.returncode == 0 and actual.stdout.strip(), 'Actual owned child process was not observable')
                        row = json.loads(actual.stdout)
                        need(row['ProcessId'] == child_pid and row['ParentProcessId'] == process.pid,
                             'Actual child is not owned by native supervisor')
                        need(Path(row['ExecutablePath']).resolve() == Path(argv[0]).resolve()
                             and claimed['test'] in row['CommandLine'] and '--ignored' in row['CommandLine'],
                             'Actual owned child program differs')
                        handle = kernel.OpenProcess(0x00100001 | 0x00001000, False, child_pid)
                        need(handle, 'Cannot retain actual owned process handle')
                        handles[role] = handle
                        records[role] = dict(role=role, claimed=claimed, actual=row,
                                             executable_sha256=sha(Path(row['ExecutablePath'])))
                        if role == 'worker':
                            need('producer' in handles and kernel.WaitForSingleObject(handles['producer'], 0) == 0, 'Producer is still alive when replacement is observed')
                            records[role]['producer_terminated_before_worker_observation'] = True
                        save(case_out / 'os-processes.json', dict(parent_pid=process.pid, children=records))
                    ready = observations / (role + '-ready.json')
                    gate = observations / permission
                    if role in records and ready.is_file() and not gate.exists():
                        try:
                            checkpoint = json.loads(ready.read_bytes())
                        except (ValueError, OSError):
                            continue
                        need(checkpoint['pid'] == records[role]['actual']['ProcessId']
                             and kernel.WaitForSingleObject(handles[role], 0) == 258,
                             'Checkpoint owner is not the actual live child')
                        if role == 'producer':
                            save(case_out / 'producer-exit-authorization.json', dict(phase='timestamp-producer-kill', writer_pid=checkpoint['pid'], supervisor_pid=process.pid, monotonic_ns=time.perf_counter_ns()))
                        save(gate, dict(os_pid=checkpoint['pid'], supervisor_pid=process.pid, monotonic_ns=time.perf_counter_ns()))
                restore_gate=observations/'allow-original-undo-restore.json'
                if 'worker' in records and not restore_gate.exists() and (observations/'worker-ready.json').is_file():
                    ready=json.loads((observations/'worker-ready.json').read_bytes())
                    pair=failed_callback(live_frames(case_out/'RM-frames.jsonl'),ready)
                    if pair is not None:
                        callback,reply=pair
                        need(kernel.WaitForSingleObject(handles['worker'],0)==258,'Failed callback worker is not live')
                        save(restore_gate,dict(xid=ready['xid'],branch_id=ready['branch_id'],failed_request_index=callback['index'],failed_response_index=reply['index'],connection=reply['connection'],worker_pid=records['worker']['actual']['ProcessId'],failed_response_sha256=hashlib.sha256(bytes.fromhex(reply['frame_hex'])).hexdigest(),monotonic_ns=time.perf_counter_ns()))
                time.sleep(.01)
            process.wait()
        terminal = {}
        need(set(handles) == {'producer', 'worker'}, 'Owned child observation incomplete')
        for role, handle in handles.items():
            need(kernel.WaitForSingleObject(handle, 0) == 0, 'Owned child remains alive')
            code = wintypes.DWORD()
            need(kernel.GetExitCodeProcess(handle, ctypes.byref(code)), 'Owned child exit status unavailable')
            terminal[role] = int(code.value)
        need(terminal['producer'] != 0 and terminal['worker'] == 0, 'Crash/recovery exit statuses differ')
        save(case_out / 'os-processes.json', dict(parent_pid=process.pid, children=records,
             native_parent_exit_code=process.returncode, terminal=terminal, all_children_stopped=True))
        return SimpleNamespace(returncode=process.returncode, stdout=stdout.read_bytes(),
                               stderr=stderr.read_bytes(), pid=process.pid)
    finally:
        # 只终止从实际父子关系确认后保留的句柄，不按可重用的外部 PID 清理。
        for handle in handles.values():
            if kernel.WaitForSingleObject(handle, 0) == 258:
                kernel.TerminateProcess(handle, 1)
                kernel.WaitForSingleObject(handle, 5000)
            kernel.CloseHandle(handle)
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=10)
