import copy
import unittest
import collect
import run
import tempfile
from pathlib import Path

def rows():
    out=[]
    for name in collect.NAMES:
        heartbeat=name.startswith('Heartbeat');kind=4 if name=='HeartbeatPong' else (3 if heartbeat else (1 if name.startswith('MergeResult') else 0))
        payload=b'' if heartbeat else (60 if name.startswith('MergeResult') else 59).to_bytes(2,'big')+b'\0\0\0\x02\0\0'
        frame=b'\xda\xda\x01'+(16+len(payload)).to_bytes(4,'big')+b'\0\x10'+bytes([kind,1,0])+b'\x10\x20\x30\x40'+payload
        out.append(dict(name=name,kind=kind,request_id=0x10203040,frame_hex=frame.hex(),serialized_hex=payload.hex()))
    return out

class CollectorGateTest(unittest.TestCase):
    def test_mid_build_edit_cannot_be_reported_as_the_compiled_input(self):
        with tempfile.TemporaryDirectory(prefix='seata-control-input-',dir='D:/Temp') as directory:
            root=Path(directory).resolve();self.assertTrue(root.is_relative_to(Path('D:/Temp').resolve()))
            source=root/'Source.java';source.write_bytes(b'original input');copied=source.read_bytes()
            source.write_bytes(b'new uncompiled input')
            with self.assertRaises(RuntimeError):run.verify_input_unchanged(source,copied)
    def test_complete_inventory_is_accepted(self):self.assertEqual(len(collect.validate_rows(rows())),9)
    def test_missing_or_duplicate_case_is_rejected(self):
        for values in [rows()[:-1],[rows()[0]]*9]:
            with self.assertRaises(ValueError):collect.validate_rows(values)
    def test_declared_length_type_and_payload_must_match_actual_frame(self):
        for field,value in [('frame_hex','dada01'),('serialized_hex','00ff'),('kind',4),('request_id',9)]:
            values=rows();values[3][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):collect.validate_rows(values)
    def test_corrupted_payload_copy_cannot_certify_the_capture(self):
        values=copy.deepcopy(rows());values[3]['frame_hex']=values[3]['frame_hex'][:-2]+'01'
        with self.assertRaises(ValueError):collect.validate_rows(values)

if __name__=='__main__':unittest.main()
