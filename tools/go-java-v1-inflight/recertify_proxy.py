"""Re-run changed proxy/Python only, binding unchanged Go/Rust execution to the core run."""
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import uuid
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def sha(data):return hashlib.sha256(data).hexdigest()
def write(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def run():
 directory=ROOT/'docs/phase-0/evidence/local-go-java-inflight';latest=json.loads((directory/'latest_run.json').read_bytes());parent=Path(latest['evidence_directory']);summary=json.loads((parent/'summary.json').read_bytes())
 if not summary['succeeded'] or summary['go_tests']<251 or summary['rust_tests']!=117:raise RuntimeError('Successful core run missing')
 out=directory/('proxy_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(exist_ok=False)
 inherited=json.loads((parent/'source-input-audit.json').read_bytes());frozen={}
 for row in inherited:
  path=ROOT/row['path'];data=path.read_bytes()
  if sha((parent/row['snapshot']).read_bytes())!=row['sha256']:raise RuntimeError('Core source snapshot was changed')
  # 本次只允许本目录Python验证工具变化，Go源码/测试/依赖与Rust/夹具仍必须逐字节相同。
  if not (path.parent==HERE and path.suffix=='.py') and sha(data)!=row['sha256']:raise RuntimeError('Core input changed, full regression required: '+row['path'])
  frozen[path]=data
 for path in HERE.glob('*.py'):frozen[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(frozen.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';write(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 write(out/'source-input-audit.json',json.dumps(rows,indent=2)+'\n')
 audit=module('proxy_recert_audit',HERE/'audit.py');sdk=audit.sdk;live=module('proxy_recert_live',HERE/'live.py');work=Path(summary['cohort'])
 if audit.run(work)!=json.loads((parent/'source-after.json').read_bytes()):raise RuntimeError('Compiled core source identity changed')
 actual=sdk.g.top(parent/'go-green.jsonl')
 if len(actual)!=summary['go_tests'] or b'117 passed; 0 failed;' not in (parent/'rust-tests.log').read_bytes():raise RuntimeError('Core execution evidence differs')
 env=sdk.h.ENV.copy();env['SEATA_INFLIGHT_COHORT']=str(work)
 sdk.l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,env,out/'python-tests.log')
 if b'Ran 5 tests' not in (out/'python-tests.log').read_bytes():raise RuntimeError('Changed proxy regression inventory differs')
 runtime=live.run(out/'live',work,Path(summary['inherited_java_evidence']))
 for path,data in frozen.items():
  if path.read_bytes()!=data:raise RuntimeError('Frozen input drift')
 result=dict(succeeded=True,core_evidence=str(parent),core_summary_sha256=sdk.h.sha(parent/'summary.json'),core_go_report_sha256=sdk.h.sha(parent/'go-green.jsonl'),inherited_go_tests=summary['go_tests'],inherited_rust_tests=117,new_go_execution=False,new_rust_execution=False,python_tests=5,source_inputs=len(rows),runtime=runtime,scope='Changed test proxy serialized complete frames per connection/direction; current Python and fresh real SDK/TC only. Go/Rust code and execution bound unchanged to core evidence.')
 write(out/'summary.json',json.dumps(result,indent=2)+'\n');(directory/'latest_proxy_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(evidence_directory=str(out),python_tests=5,live=runtime['wire_inventory'])),flush=True)
if __name__=='__main__':run()
