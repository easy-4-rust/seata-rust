"""Exact Go source closure: original714, certified0001..0006, current0007 and its tests."""
from pathlib import Path
import hashlib,importlib.util,json
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
parent=module('identity_audit_parent',ROOT/'tools/go-java-v1-inflight/audit.py');prepare=module('identity_audit_prepare',HERE/'prepare_candidate.py')
def sha(data):return hashlib.sha256(data).hexdigest()
def run(work):
 original_parent=parent.run(prepare.PARENT);content=prepare.files();expected={row['path']:row['patched_sha256'] for row in original_parent['candidate_files']};expected.update({path:sha(data) for path,data in content.items()})
 originals=parent.sdk.l.audit_cohort(work,expected,(ROOT/'tools/go-java-v1-length-patch/java_v1_length_compat_test.go').read_bytes())
 for path,data in content.items():
  if (work/path).read_bytes()!=data:raise RuntimeError('Actual identity candidate source differs: '+path)
 auxiliary={row['path']:row['sha256'] for row in original_parent['auxiliary']}
 for source in HERE.glob('*_test.go'):auxiliary['pkg/datasource/sql/'+source.name]=sha(source.read_bytes())
 for path,digest in auxiliary.items():
  if sha((work/path).read_bytes())!=digest:raise RuntimeError('Actual identity test source differs: '+path)
 wanted={row['path'] for row in originals if row['path'].endswith('.go')}|set(expected)|set(auxiliary);actual={p.relative_to(work).as_posix() for p in work.rglob('*.go')}
 if wanted!=actual:raise RuntimeError('Unexpected/missing Go path: '+str(wanted^actual))
 return dict(original_members=originals,candidate_files=[dict(path=path,sha256=digest) for path,digest in sorted(expected.items())],auxiliary=[dict(path=path,sha256=digest) for path,digest in sorted(auxiliary.items())])
