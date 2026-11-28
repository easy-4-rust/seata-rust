"""Observe owned Rust children in Windows before allowing crash or TC End."""
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import subprocess
import time
from types import SimpleNamespace

from inventory_rebuilt import need, save, sha


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
