"""Reproduce pinned SDK RED, fresh0006 Go/Rust regression and actual TC response loss."""
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import uuid
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
def write(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 with path.open('xb') as stream:stream.write(data.encode() if isinstance(data,str) else data)
def run():
 stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8];out=ROOT/'docs/phase-0/evidence/local-go-java-inflight'/('run_'+stamp);out.mkdir(parents=True,exist_ok=False)
 parent=ROOT/'docs/phase-0/evidence/local-java-preface-candidate/run_20261112_221159_a16eafb1';authority=json.loads((parent/'summary.json').read_bytes())
 if not authority['succeeded'] or authority['java_selected_tests']!=617:raise RuntimeError('Certified Java parent missing')
 # 先冻结源输入，再导入本候选及其继承Python支持，避免事后快照冒充已执行字节。
 previous=json.loads((parent/'source-input-audit.json').read_bytes());frozen={}
 for row in previous:
  path=ROOT/row['path'];data=path.read_bytes()
  if sha(data)!=row['sha256'] or sha((parent/row['snapshot']).read_bytes())!=row['sha256']:raise RuntimeError('Certified inherited input drift: '+row['path'])
  frozen[path]=data
 paths=[p for p in HERE.iterdir() if p.is_file() and p.suffix in ['.go','.py','.txt']]
 paths += [ROOT/'docs/phase-0/go-java-v1-inflight-plan.md']
 patch_dir=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498';paths += [patch_dir/'0006-java-v1-inflight-outcomes.patch',patch_dir/'inflight_manifest.json']
 for path in paths:frozen[path]=path.read_bytes()
 rows=[]
 for path,data in sorted(frozen.items()):
  name=sha(path.relative_to(ROOT).as_posix().encode())[:20]+'.bin';write(out/'source-inputs'/name,data);rows.append(dict(path=path.relative_to(ROOT).as_posix(),snapshot='source-inputs/'+name,sha256=sha(data)))
 write(out/'source-input-audit.json',json.dumps(rows,indent=2)+'\n')
 audit=module('inflight_fresh_audit',HERE/'audit.py');sdk=audit.sdk;prepare=audit.prepare;live=module('inflight_fresh_live',HERE/'live.py');h=sdk.h;l=sdk.l
 if h.sha(sdk.c.ARCHIVE)!='a2876b1118b267ffbf7d73db8e0ef8f18477ae4d2b700944ebcca11871b6db1f':raise RuntimeError('Original Go archive differs')
 work=Path('D:/codex-builds')/('sif_'+stamp);work.mkdir(exist_ok=False)
 with tarfile.open(sdk.c.ARCHIVE,'r:gz') as source:
  for entry in source:
   if not entry.isfile():continue
   relative='/'.join(entry.name.split('/')[1:]);target=work/relative
   if not target.resolve().is_relative_to(work.resolve()):raise RuntimeError('Archive escape')
   target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(source.extractfile(entry).read())
 for patch in sorted(patch_dir.glob('000[1-5]-*.patch')):sdk.apply(patch,work,out,patch.stem)
 for relative,path in sdk.copies().items():
  target=work/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(frozen[path])
 test=HERE/'java_v1_inflight_test.go';(work/'pkg/remoting/getty'/test.name).write_bytes(frozen[test])
 original=audit.run(work,include_candidate=False);write(out/'source-before.json',json.dumps(original,indent=2)+'\n')
 code=l.run_command(['go','test','./pkg/remoting/getty','-run','^TestJavaV1Inflight','-count=1','-timeout=10s','-json'],work,sdk.c.GO_ENV,out/'go-red.jsonl',timeout=20,expected=None)
 red=[json.loads(line) for line in (out/'go-red.jsonl').read_bytes().splitlines() if line.startswith(b'{')];failures={r['Test'] for r in red if r.get('Action')=='fail' and r.get('Test')}
 required={'TestJavaV1InflightClosedSessionIsLocal','TestJavaV1InflightEncodingFailureIsLocal','TestJavaV1InflightWriteFailureIsUnknownEvenWhenZero','TestJavaV1InflightTimeoutIsUnknownAndLateReplyDoesNotRevive','TestJavaV1InflightCloseOnlyWakesOwnedSession','TestJavaV1InflightErrorDuringWritePreservesCause','TestJavaV1InflightWrongSessionCannotCompleteRequest','TestJavaV1InflightForeignReplyDisguisedAsRequestIsIgnored','TestJavaV1InflightDuplicateIDPreservesExistingFuture','TestJavaV1InflightPausedOldProcessorCannotCompleteReusedID'}
 if code!=1 or not required.issubset(failures):raise RuntimeError('Behavioral RED differs')
 if audit.run(work,include_candidate=False)!=original:raise RuntimeError('Original candidate changed during RED')
 sdk.apply(prepare.PATCH,work,out,'0006');candidate=audit.run(work);write(out/'source-after.json',json.dumps(candidate,indent=2)+'\n')
 go_env=sdk.c.GO_ENV.copy();go_env.update(SEATA_GETTY_VECTORS=str(parent/'vectors.json'),SEATA_MERGE_VECTORS=str(parent/'capture/merge/vectors.json'),SEATA_EXCEPTION_VECTORS=str(parent/'capture/exception/vectors.json'),SEATA_JAVA_LENGTH_VECTORS=str(parent/'capture/length/vectors.json'))
 l.run_command(['go','test',*sdk.PACKAGES,'-count=1','-timeout=210s','-json'],work,go_env,out/'go-green.jsonl',timeout=240)
 actual=sdk.g.top(out/'go-green.jsonl');old=sdk.g.top(parent/'go-tests.jsonl')
 if len(actual)!=254 or not old.issubset(actual):raise RuntimeError('Go254 identities differ')
 env={k:v for k,v in h.ENV.items() if not k.startswith('SEATA_IT_')};env.update(SEATA_IT_CONTROL_WIRE_DIR=str(parent/'capture/control'),SEATA_IT_MERGE_PROFILE_DIR=str(parent/'capture/merge'),SEATA_IT_EXCEPTION_PROFILE_DIR=str(parent/'capture/exception'),SEATA_IT_ERROR_LENGTH_DIR=str(parent/'capture/length'),SEATA_IT_HANDLER_WIRE_DIR=str(ROOT/'tests/fixtures/protocol/v1/handler_matrix'),SEATA_INFLIGHT_COHORT=str(work))
 cargo=ROOT/'tools/java-tc-integration/rust-client/Cargo.toml'
 for label,args in [('rust-tests',['cargo','test','--manifest-path',cargo]),('rust-clippy',['cargo','clippy','--manifest-path',cargo,'--all-targets','--','-D','warnings']),('rust-fmt',['cargo','fmt','--manifest-path',cargo,'--','--check'])]:l.run_command(args,ROOT,env,out/(label+'.log'))
 if b'117 passed; 0 failed;' not in (out/'rust-tests.log').read_bytes():raise RuntimeError('Rust117 inventory differs')
 l.run_command(['python','-m','unittest','discover','-s',HERE,'-p','test_*.py','-v'],ROOT,env,out/'python-tests.log')
 runtime=live.run(out/'live',work,parent)
 if audit.run(work)!=candidate:raise RuntimeError('Executed candidate changed')
 sdk.audit_candidate(Path(authority['go_cohort']));sdk.c.audit_go()
 for path,data in frozen.items():
  if path.read_bytes()!=data:raise RuntimeError('Frozen input drift: '+str(path))
 if h.sha(sdk.c.ORACLE/'server/target/seata-server.jar')!=h.JAR_HASH:raise RuntimeError('Original Java JAR drift')
 summary=dict(succeeded=True,completed_at=datetime.datetime.now().astimezone().isoformat(),cohort=str(work),inherited_java_evidence=str(parent),inherited_java_summary_sha256=h.sha(parent/'summary.json'),inherited_java_tests=617,new_java_execution=False,go_tests=254,inherited_go_tests=235,new_go_tests=19,rust_tests=117,python_tests=5,red_failures=sorted(failures),red_passes=sum(r.get('Action')=='pass' and bool(r.get('Test')) for r in red),patch_sha256=h.sha(prepare.PATCH),runtime=runtime,scope='Independent explicit synchronous SDK outcomes/session correlation candidate. Original/default behavior, async/business-client lifecycle, full RM/Undo, Begin idempotency, production Rust and WP00 remain open.')
 write(out/'summary.json',json.dumps(summary,indent=2)+'\n');(out.parent/'latest_run.json').write_text(json.dumps(dict(succeeded=True,evidence_directory=str(out)),indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(evidence_directory=str(out),go_tests=254,rust_tests=117,live=runtime['wire_inventory'])),flush=True)
if __name__=='__main__':run()
