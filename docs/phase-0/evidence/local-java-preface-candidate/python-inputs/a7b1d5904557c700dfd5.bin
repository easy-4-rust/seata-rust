"""Actual JDK class-load tag parsing and overlay provenance guards; D: scratch only."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
HERE=Path(__file__).resolve().parent
runtime=module('preface_provenance_runtime',HERE/'runtime.py');prepare=module('preface_provenance_prepare',HERE/'prepare_candidate.py')

class ProvenanceTest(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='preface-origin-',dir='D:/Temp');self.directory=Path(self.temp.name);self.log=self.directory/'load.log';self.overlay=self.directory/'overlay'
 def tearDown(self):
  assert self.directory.resolve().is_relative_to(Path('D:/Temp').resolve());self.temp.cleanup()
 def lines(self,padding='',origin=None):
  url=origin or 'file:/'+self.overlay.as_posix()+'/'
  return ''.join('[1.234s][info][class,load'+padding+'] '+relative.split('src/main/java/',1)[1][:-5].replace('/','.')+' source: '+url+'\n' for name,relative in prepare.FILES.items() if name.endswith('.java'))
 def test_actual_padded_jdk_tags_are_accepted(self):
  self.log.write_text(self.lines('  '),encoding='utf-8');self.assertEqual(len(runtime.class_sources(self.log,self.overlay,prepare)),5)
 def test_unpadded_tags_are_accepted(self):
  self.log.write_text(self.lines(),encoding='utf-8');self.assertEqual(len(runtime.class_sources(self.log,self.overlay,prepare)),5)
 def test_wrong_origin_and_missing_or_duplicate_class_are_rejected(self):
  correct=self.lines('  ')
  for text in [self.lines('  ','jar:file:/unpatched.jar!/'),correct.split('\n',1)[1],correct+correct.splitlines()[0]+'\n']:
   self.log.write_text(text,encoding='utf-8')
   with self.subTest(text=text[:80]),self.assertRaises(RuntimeError):runtime.class_sources(self.log,self.overlay,prepare)

if __name__=='__main__':unittest.main()
