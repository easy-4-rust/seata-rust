"""Read-only Windows process handle observation; never terminates a process by PID."""
import ctypes
from ctypes import wintypes

from inventory_rebuilt import need


kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel.OpenProcess.restype = wintypes.HANDLE
kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel.WaitForSingleObject.restype = wintypes.DWORD
kernel.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
kernel.GetExitCodeProcess.restype = wintypes.BOOL
kernel.CloseHandle.argtypes = [wintypes.HANDLE]
kernel.CloseHandle.restype = wintypes.BOOL


def open_writer(pid):
    need(type(pid) is int and pid > 0, 'Invalid writer PID')
    handle = kernel.OpenProcess(0x100000 | 0x1000, False, pid)
    need(bool(handle), 'Cannot observe actual owned writer handle')
    return handle


def state(handle):
    waited = kernel.WaitForSingleObject(handle, 0)
    need(waited in [0, 258], 'Writer handle observation failed')
    code = wintypes.DWORD()
    need(kernel.GetExitCodeProcess(handle, ctypes.byref(code)), 'Writer exit status unavailable')
    return dict(alive=waited == 258, exit_code=code.value)


def close(handle):
    need(kernel.CloseHandle(handle), 'Writer observer handle did not close')
