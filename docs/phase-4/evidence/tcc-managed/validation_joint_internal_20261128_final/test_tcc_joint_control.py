"""Actual Windows file-sharing and producer-liveness checks for published control observations."""
import ctypes,json,subprocess,sys,tempfile,threading,unittest
from ctypes import wintypes
from pathlib import Path
from tcc_joint_crash import published_json

class PublishedControlTests(unittest.TestCase):
    def test_exclusive_windows_file_lock_retries_same_published_bytes(self):
        kernel=ctypes.WinDLL('kernel32',use_last_error=True)
        kernel.CreateFileW.argtypes=[wintypes.LPCWSTR,wintypes.DWORD,wintypes.DWORD,ctypes.c_void_p,wintypes.DWORD,wintypes.DWORD,wintypes.HANDLE];kernel.CreateFileW.restype=wintypes.HANDLE
        kernel.CloseHandle.argtypes=[wintypes.HANDLE];kernel.CloseHandle.restype=wintypes.BOOL
        with tempfile.TemporaryDirectory(dir='E:/codex-builds') as tmp:
            path=Path(tmp)/'published.json';raw=b'{"original":"same bytes"}\n';path.write_bytes(raw)
            handle=kernel.CreateFileW(str(path),0x80000000,0,None,3,0x80,None)
            self.assertNotEqual(handle,wintypes.HANDLE(-1).value)
            try:
                with self.assertRaises(PermissionError):path.read_bytes()
                timer=threading.Timer(.08,lambda:kernel.CloseHandle(handle));timer.start()
                actual,value=published_json(path,timeout=2);timer.join();handle=None
                self.assertEqual(actual,raw);self.assertEqual(value,{'original':'same bytes'})
            finally:
                if handle is not None:kernel.CloseHandle(handle)
    def test_partial_json_is_not_a_complete_observation(self):
        with tempfile.TemporaryDirectory(dir='E:/codex-builds') as tmp:
            path=Path(tmp)/'partial.json';path.write_bytes(b'{')
            with self.assertRaises(ValueError):published_json(path,timeout=.05)
    def test_completed_state_file_cannot_replace_live_producer(self):
        with tempfile.TemporaryDirectory(dir='E:/codex-builds') as tmp:
            path=Path(tmp)/'published.json';path.write_text(json.dumps({'state':'ready'}),encoding='utf-8')
            process=subprocess.Popen([sys.executable,'-B','-c','pass'],cwd=tmp,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW);process.wait(timeout=5)
            with self.assertRaises(ValueError):published_json(path,owners=(process,))

if __name__=='__main__':unittest.main()
