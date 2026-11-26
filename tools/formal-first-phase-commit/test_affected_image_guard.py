"""Bounded fault transformer tests; final acceptance uses original captured packets."""
import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"formal-tc-lock-coverage"))
from mysql_affected_image_guard import affected_count_fault,UPDATE,DELETE

OK=bytes((7,0,0,1,0,0,0,3,0,4,0))

class AffectedImageFaultTest(unittest.TestCase):
    def test_only_the_affected_byte_changes_and_both_dml_targets_are_supported(self):
        for sql in (UPDATE,DELETE):
            with self.subTest(sql=sql):
                actual=affected_count_fault(sql,OK)
                self.assertEqual(actual[:5],OK[:5])
                self.assertEqual(actual[5],1)
                self.assertEqual(actual[6:],OK[6:])
    def test_other_statements_preserve_the_original_packet_object(self):
        self.assertIs(affected_count_fault("INSERT unrelated",OK),OK)
        self.assertEqual(affected_count_fault("UPDATE unrelated",b"not inspected"),b"not inspected")
    def test_nonzero_count_insert_id_error_length_and_sequence_are_rejected(self):
        for position,value in ((0,8),(3,2),(4,255),(5,1),(6,1)):
            wire=bytearray(OK);wire[position]=value
            with self.subTest(position=position),self.assertRaises(AssertionError):
                affected_count_fault(UPDATE,bytes(wire))
        with self.assertRaises(AssertionError):
            affected_count_fault(UPDATE,b"short")

if __name__=="__main__":
    unittest.main()
