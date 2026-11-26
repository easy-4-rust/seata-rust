"""Reproducible patch metadata must preserve existing bytes across Windows newlines."""
import tempfile,unittest
from pathlib import Path
from prepare_candidate import put,run

class CandidateArtifactGuards(unittest.TestCase):
 def test_crlf_metadata_remains_byte_identical(self):
  with tempfile.TemporaryDirectory(dir='D:/Temp',prefix='retention-metadata-') as directory:
   path=Path(directory)/'manifest.json';before=b'{\r\n  "a": 1\r\n}\r\n';path.write_bytes(before);put(path,b'{\n  "a": 1\n}\n');self.assertEqual(path.read_bytes(),before)
 def test_different_metadata_is_not_overwritten(self):
  with tempfile.TemporaryDirectory(dir='D:/Temp',prefix='retention-metadata-') as directory:
   path=Path(directory)/'manifest.json';before=b'{"a":1}\n';path.write_bytes(before)
   with self.assertRaises(RuntimeError):put(path,b'{"a":2}\n')
   self.assertEqual(path.read_bytes(),before)
 def test_current_patch_reproduces_exactly(self):self.assertEqual(len(run()['files']),3)

if __name__=='__main__':unittest.main()
