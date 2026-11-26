"""Diagnostic logger output must never be mistaken for the protobuf reader result."""
import unittest
from evidence import parse_metadata
class MetadataOutputGuards(unittest.TestCase):
 def test_logger_prefix_does_not_corrupt_result(self):
  self.assertEqual(parse_metadata(b'WARN [main] logging fallback\nSNAPSHOT_METADATA_JSON=[{"index":12,"term":2}]\n'),[dict(index=12,term=2)])
 def test_missing_reader_result_rejected(self):
  with self.assertRaises(RuntimeError):parse_metadata(b'INFO [main] []\n')
 def test_multiple_reader_results_rejected(self):
  with self.assertRaises(RuntimeError):parse_metadata(b'SNAPSHOT_METADATA_JSON=[]\nSNAPSHOT_METADATA_JSON=[]\n')
if __name__=='__main__':unittest.main()
