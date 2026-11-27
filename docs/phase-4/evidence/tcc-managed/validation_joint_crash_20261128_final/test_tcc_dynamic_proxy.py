"""Pure observer-rule checks using sealed original TC/Rust frames; not a network success gate."""
import json
from pathlib import Path
import struct
import tempfile
import threading
import time
import unittest
from rm_proxy import identity
from tcc_dynamic_proxy import DynamicRule

FIXTURE=json.loads(Path(__file__).with_name('tcc_dynamic_proxy_frames.json').read_bytes())
FRAMES={int(k):bytes.fromhex(v) for k,v in FIXTURE['frames'].items()}

def rule(mode):
    value=DynamicRule(mode)
    value.inspect('c2s',1,FRAMES[103]);value.inspect('s2c',1,FRAMES[104])
    return value


class DynamicProxyTests(unittest.TestCase):
    def test_canonical_zero_length_optional_resource_report_and_original_success_are_required(self):
        value=rule('report_lost')
        event,sends,closed=value.inspect('c2s',1,FRAMES[13])
        self.assertEqual(sends,[FRAMES[13]]);self.assertFalse(closed)
        event,sends,closed=value.inspect('s2c',1,FRAMES[14])
        self.assertEqual(event['action'],'drop_and_close');self.assertEqual(sends,[]);self.assertTrue(closed)
        self.assertEqual((value.faults,value.closed),(1,1))

    def test_AT_phase_one_status_cannot_trigger_report_fault(self):
        frame=bytearray(FRAMES[13]);head=int.from_bytes(frame[7:9],'big');_,_,end=identity(frame[head:],2);frame[head+end]=2
        value=rule('report_lost')
        with self.assertRaises(ValueError):value.inspect('c2s',1,bytes(frame))
        self.assertEqual(value.faults,0)

    def test_unmatched_or_failed_response_is_never_claimed_as_original_success(self):
        value=rule('report_lost')
        with self.assertRaises(ValueError):value.inspect('s2c',1,FRAMES[14])
        value.inspect('c2s',1,FRAMES[13]);frame=bytearray(FRAMES[14]);frame[-2]=0
        with self.assertRaises(ValueError):value.inspect('s2c',1,bytes(frame))
        self.assertEqual(value.faults,0)

    def test_duplicate_callback_still_uses_only_two_copies_of_original_frame(self):
        value=rule('duplicate');_,sends,closed=value.inspect('s2c',1,FRAMES[5])
        self.assertEqual(sends,[FRAMES[5],FRAMES[5]]);self.assertFalse(closed)
        _,sends,_=value.inspect('c2s',1,FRAMES[6]);self.assertEqual(sends,[])
        _,sends,_=value.inspect('c2s',1,FRAMES[6]);self.assertEqual(sends,[FRAMES[6]])
        self.assertEqual((value.faults,value.released),(1,1))

    def test_report_waiter_retains_original_success_until_external_cancellation_observation(self):
        with tempfile.TemporaryDirectory(prefix='seata-dynamic-proxy-',dir='D:/Temp') as directory:
            base=Path(directory);held=base/'held.json';release=base/'release'
            value=DynamicRule('report_cancel',held,release);value.inspect('c2s',1,FRAMES[103]);value.inspect('s2c',1,FRAMES[104])
            value.inspect('c2s',1,FRAMES[13]);result=[]
            worker=threading.Thread(target=lambda:result.append(value.inspect('s2c',1,FRAMES[14])));worker.start()
            try:
                limit=time.monotonic()+3
                while not held.exists():
                    self.assertLess(time.monotonic(),limit);time.sleep(.01)
                self.assertTrue(worker.is_alive());self.assertEqual(result,[])
                self.assertEqual(json.loads(held.read_bytes())['frame_hex'],FRAMES[14].hex())
            finally:release.write_bytes(b'close-after-cancel');worker.join(timeout=3)
            self.assertFalse(worker.is_alive());self.assertEqual(result[0][1:],([],True))


if __name__=='__main__':unittest.main()
