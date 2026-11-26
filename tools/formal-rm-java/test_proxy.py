"""Bounded raw-frame duplicate/loss controls must preserve TC identity and actual delivery counts."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec=importlib.util.spec_from_file_location('formal_rm_callback_proxy',Path(__file__).with_name('rm_proxy.py'))
proxy=importlib.util.module_from_spec(spec)
spec.loader.exec_module(proxy)

def text(value):
    value=value.encode();return struct.pack('>h',len(value))+value
def frame(kind,request_type,request_id,payload=b''):
    body=struct.pack('>H',kind)+payload
    return b'\xda\xda\x01'+struct.pack('>IHB',16+len(body),16,request_type)+b'\x01\x00'+struct.pack('>i',request_id)+body
def callback(request_id=-7,kind=3):
    return frame(kind,0,request_id,text('tc:9007199254740993')+struct.pack('>q',9007199254740995)+b'\x00'+text('mysql://auth-a')+struct.pack('>i',2)+b'{}')
def reply(request_id=-7,kind=4,status=5,xid='tc:9007199254740993'):
    return frame(kind,1,request_id,b'\x01\x00'+text(xid)+struct.pack('>qB',9007199254740995,status))

class RuleTests(unittest.TestCase):
    def rule(self,mode):
        value=proxy.CallbackRule(mode);value.inspect('c2s',1,frame(103,0,1));return value
    def test_basic_forwards_the_exact_original_bytes(self):
        value=self.rule('basic');raw=callback();event,sends,closed=value.inspect('s2c',1,raw)
        self.assertEqual(sends,[raw]);self.assertFalse(closed);self.assertEqual(event['action'],'forward')
        self.assertEqual(value.inspect('c2s',1,reply())[1],[reply()]);self.assertEqual(value.faults,0)
    def test_duplicate_waits_for_two_actual_replies_before_releasing_one_original_to_tc(self):
        value=self.rule('duplicate');raw=callback();self.assertEqual(value.inspect('s2c',1,raw)[1],[raw,raw])
        event,sends,closed=value.inspect('c2s',1,reply());self.assertEqual(sends,[]);self.assertEqual(event['action'],'hold');self.assertFalse(closed)
        event,sends,closed=value.inspect('c2s',1,reply());self.assertEqual(sends,[reply()]);self.assertEqual(event['action'],'release_original');self.assertFalse(closed);self.assertEqual(value.released,1)
    def test_duplicate_does_not_inject_into_a_second_native_callback(self):
        value=self.rule('duplicate');value.inspect('s2c',1,callback());self.assertEqual(value.inspect('s2c',1,callback(-8))[1],[callback(-8)]);self.assertEqual(value.faults,1)
    def test_reconnect_drops_one_confirmed_reply_and_closes_only_that_session(self):
        value=self.rule('reconnect');value.inspect('s2c',1,callback());event,sends,closed=value.inspect('c2s',1,reply());self.assertEqual(sends,[]);self.assertTrue(closed);self.assertEqual(event['action'],'drop_and_close')
        value.inspect('c2s',2,frame(103,0,1));value.inspect('s2c',2,callback(-8));self.assertEqual(value.inspect('c2s',2,reply(-8))[1],[reply(-8)]);self.assertEqual(value.faults,1)
    def test_retryable_status_does_not_arm_successful_reply_loss(self):
        value=self.rule('reconnect');value.inspect('s2c',1,callback());event,sends,closed=value.inspect('c2s',1,reply(status=6));self.assertEqual(sends,[reply(status=6)]);self.assertFalse(closed);self.assertEqual(value.faults,0)
    def test_unmatched_reply_or_wrong_xid_is_rejected(self):
        value=self.rule('basic')
        with self.assertRaises(ValueError):value.inspect('c2s',1,reply())
        value.inspect('s2c',1,callback())
        with self.assertRaises(ValueError):value.inspect('c2s',1,reply(xid='changed'))
    def test_duplicate_reply_changed_between_deliveries_cannot_be_released(self):
        value=self.rule('duplicate');value.inspect('s2c',1,callback());value.inspect('c2s',1,reply())
        with self.assertRaises(ValueError):value.inspect('c2s',1,reply(status=6))
    def test_callback_reusing_same_id_with_changed_original_is_rejected(self):
        value=self.rule('basic');value.inspect('s2c',1,callback())
        with self.assertRaises(ValueError):value.inspect('s2c',1,callback(kind=5))
    def test_limits_reject_large_length_before_reading_the_body(self):
        import io
        header=b'\xda\xda\x01'+struct.pack('>IHB',65537,16,0)+b'\x01\x00'+struct.pack('>i',1)
        with self.assertRaises(ValueError):proxy.read_frame(io.BytesIO(header))
    def test_missing_payload_and_unselected_mode_are_rejected(self):
        with self.assertRaises(ValueError):proxy.CallbackRule('unknown')
        with self.assertRaises(ValueError):self.rule('basic').inspect('s2c',1,b'bad')
    def test_tm_observer_is_separate_passthrough_and_never_injects_an_rm_fault(self):
        value=proxy.CallbackRule('basic','TM');raw=frame(101,0,1);self.assertEqual(value.inspect('c2s',1,raw)[1],[raw]);self.assertEqual(value.roles,{1:'TM'});self.assertEqual(value.faults,0)
        with self.assertRaises(ValueError):proxy.CallbackRule('reconnect','TM')
    def test_tm_observer_rejects_rm_callbacks_and_rm_registration(self):
        value=proxy.CallbackRule('basic','TM')
        with self.assertRaises(ValueError):value.inspect('s2c',1,callback())
        with self.assertRaises(ValueError):value.inspect('c2s',1,frame(103,0,1))

if __name__=='__main__':unittest.main()
