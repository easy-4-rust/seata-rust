"""Final Go production/test/template closure after0001..0006, with no unlisted .go files."""
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
sdk=module('inflight_audit_sdk',ROOT/'tools/go-java-v1-sdk-profile/run.py');prepare=module('inflight_audit_prepare',HERE/'prepare_candidate.py')
def run(work,include_candidate=True):
 paths=[sdk.l.PATCH_DIR/'manifest.json',sdk.e.MANIFEST,sdk.m.MANIFEST,sdk.g.MANIFEST,sdk.MANIFEST]+([prepare.MANIFEST] if include_candidate else [])
 expected={row['path']:row for path in paths for row in json.loads(path.read_bytes())['files']}
 for path,patch in zip(paths,[sdk.l.PATCH,sdk.e.PATCH,sdk.m.PATCH,sdk.g.PATCH,sdk.PATCH,prepare.PATCH]):
  manifest=json.loads(path.read_bytes())
  if sdk.h.sha(patch)!=manifest['patch_sha256']:raise RuntimeError('Patch identity changed: '+str(patch))
  for name,digest in manifest.get('requires',{}).items():
   if sdk.h.sha(path.parent/name)!=digest:raise RuntimeError('Patch dependency identity changed: '+name)
 original=sdk.l.audit_cohort(work,{path:row['patched_sha256'] for path,row in expected.items()},(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
 for path,row in expected.items():
  if sdk.h.sha(work/path)!=row['patched_sha256']:raise RuntimeError('Candidate file differs: '+path)
 for path,data in {**sdk.g.prep.files(),**sdk.prep.files(),**(prepare.files() if include_candidate else {})}.items():
  if (work/path).read_bytes()!=data:raise RuntimeError('Executed/template bytes differ: '+path)
 auxiliary={**sdk.copies(),'pkg/remoting/getty/java_v1_inflight_test.go':HERE/'java_v1_inflight_test.go'}
 for path,source in auxiliary.items():
  if (work/path).read_bytes()!=source.read_bytes():raise RuntimeError('Executed test/harness differs: '+path)
 wanted={row['path'] for row in original if row['path'].endswith('.go')}|set(expected)|set(auxiliary)
 actual={path.relative_to(work).as_posix() for path in work.rglob('*.go')}
 if wanted!=actual:raise RuntimeError('Go path closure differs: '+str(wanted^actual))
 return dict(original_members=original,candidate_files=list(expected.values()),auxiliary=[dict(path=path,sha256=sdk.h.sha(work/path)) for path in sorted(auxiliary)])
