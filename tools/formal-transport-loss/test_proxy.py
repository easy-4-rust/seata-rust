"""验证原始字节观察器的丢帧前提，不导入被测Rust codec生成预期。"""
import io
import struct
import unittest
from loss_proxy import DropRule,read_frame
from live import lock_results


def string(value):
    raw=value.encode();return struct.pack('>h',len(raw))+raw
def frame(kind,body=b'',request_type=0,request_id=1):
    payload=struct.pack('>H',kind)+body
    return b'\xda\xda\x01'+struct.pack('>IHBBB',16+len(payload),16,request_type,1,0)+struct.pack('>i',request_id)+payload


class ProxyControls(unittest.TestCase):
    def test_successful_correlated_begin_is_dropped_once_without_rewriting(self):
        rule=DropRule(2);request=frame(1,b'begin');rule.inspect('c2s',0,request)
        response=frame(2,b'\x01\x00'+string('original-xid'),1)
        row=rule.inspect('s2c',0,response);self.assertTrue(row['dropped']);self.assertEqual(row['xid'],'original-xid');self.assertEqual(row['frame_hex'],response.hex())
        self.assertFalse(rule.inspect('s2c',0,response)['dropped']);self.assertEqual(rule.drop_count,1)
    def test_failed_response_is_not_withheld_as_success(self):
        rule=DropRule(2);rule.inspect('c2s',0,frame(1))
        self.assertFalse(rule.inspect('s2c',0,frame(2,b'\x00\x00'+string('x'),1))['dropped']);self.assertEqual(rule.drop_count,0)
    def test_wrong_pending_operation_or_missing_pending_cannot_trigger_drop(self):
        rule=DropRule(2);response=frame(2,b'\x01\x00'+string('x'),1)
        self.assertFalse(rule.inspect('s2c',0,response)['dropped']);rule.inspect('c2s',0,frame(7,string('x')))
        self.assertFalse(rule.inspect('s2c',0,response)['dropped'])
    def test_signed_id_remains_signed_in_correlated_trace(self):
        rule=DropRule(2);rule.inspect('c2s',0,frame(1,request_id=-1))
        row=rule.inspect('s2c',0,frame(2,b'\x01\x00'+string('x'),1,-1));self.assertEqual(row['request_id'],-1);self.assertTrue(row['dropped'])
    def test_frame_reader_preserves_complete_frame_and_rejects_truncation(self):
        raw=frame(1,b'body');self.assertEqual(read_frame(io.BytesIO(raw)),raw);self.assertIsNone(read_frame(io.BytesIO(b'')))
        with self.assertRaises(EOFError):read_frame(io.BytesIO(raw[:-1]))
    def test_callback_identity_keeps_exact_i64_above_float_precision(self):
        rule=DropRule(8);branch=9007199254740993
        callback=rule.inspect('s2c',1,frame(3,string('x')+struct.pack('>q',branch)+b'\x00'+string('r')+b'\x00\x00\x00\x00',0,-7))
        reply=rule.inspect('c2s',1,frame(4,b'\x01\x00'+string('x')+struct.pack('>q',branch)+b'\x04',1,-7))
        self.assertEqual(callback['branch_id'],branch);self.assertEqual(reply['branch_id'],branch);self.assertEqual(reply['request_id'],-7)
    def test_lock_observer_reads_both_bytes_of_java_u16_boolean(self):
        rows=[dict(frame_hex=frame(22,b'\x01\x00'+struct.pack('>H',value),1).hex()) for value in [0,1]]
        self.assertEqual(lock_results(rows),[0,1])


if __name__=='__main__':unittest.main()
