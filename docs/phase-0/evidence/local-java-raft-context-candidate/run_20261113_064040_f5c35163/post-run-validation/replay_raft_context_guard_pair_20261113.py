from pathlib import Path
import hashlib,json,os,subprocess,tempfile
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');HERE=ROOT/'tools/go-java-v1-raft-context-update';OUT=Path('D:/codex-builds/raft-context-python-guard-pair-20261113');OUT.mkdir(exist_ok=True)
def put(p,data):
 data=data.encode() if isinstance(data,str) else data
 if p.exists():assert p.read_bytes()==data
 else:p.write_bytes(data)
raw=(HERE/'test_updated_context.py').read_bytes();implementation=(HERE/'context_evidence.py').read_bytes();put(OUT/'test_updated_context.py',raw);put(OUT/'candidate-context-evidence.py',implementation);results={}
with tempfile.TemporaryDirectory(prefix='context-guard-pair-',dir='D:/Temp') as value:
 temp=Path(value);assert temp.resolve().is_relative_to(Path('D:/Temp').resolve())
 for label in ['red','green']:
  folder=temp/label;folder.mkdir();(folder/'test_updated_context.py').write_bytes(raw)
  adapter='def verify_updated_context(*args,**kwargs):return {}\n' if label=='red' else 'import importlib.util\nfrom pathlib import Path\ns=importlib.util.spec_from_file_location("actual_guard",Path('+repr(str(HERE/'context_evidence.py'))+'))\nm=importlib.util.module_from_spec(s);s.loader.exec_module(m)\nverify_updated_context=m.verify_updated_context\n'
  (folder/'context_evidence.py').write_text(adapter,encoding='utf-8');put(OUT/(label+'-adapter.py'),adapter);run=subprocess.run(['python','-m','unittest','discover','-s',str(folder),'-p','test_updated_context.py','-v'],cwd=folder,env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','PYTHONUTF8':'1','TEMP':'D:/Temp','TMP':'D:/Temp'},stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=20)
  assert b'Ran 15 tests' in run.stdout
  if label=='red':assert run.returncode and b'FAILED (failures=14)' in run.stdout
  else:assert run.returncode==0 and run.stdout.rstrip().endswith(b'OK')
  if not (OUT/(label+'.log')).exists():put(OUT/(label+'.log'),run.stdout)
  results[label]=dict(exit_code=run.returncode,tests=15,failures=14 if label=='red' else 0,controls=1 if label=='red' else 15,test_source_sha256=hashlib.sha256(raw).hexdigest())
assert (HERE/'test_updated_context.py').read_bytes()==raw and (HERE/'context_evidence.py').read_bytes()==implementation
put(OUT/'result.json',json.dumps(dict(succeeded=True,same_test_bytes=True,results=results),indent=2)+'\n');print(json.dumps(results))
