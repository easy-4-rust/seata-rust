import hashlib
import importlib.util
import json
from pathlib import Path
import sys

ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation')
GATE=Path(sys.argv[1])
OUT=ROOT/'docs/phase-2/evidence/undo-codec-gate'/('audit_'+GATE.name)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)
def save(name,value):
    p=OUT/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def catalog(path):
    for row in read(path/'catalog.json')['members']:need(sha(path/row['path'])==row['sha256'],'Sealed archive differs: '+row['path'])
    return dict(path=str(path),catalog_sha256=sha(path/'catalog.json'))

OUT.mkdir(exist_ok=False)
(OUT/'audit.py').write_bytes(Path(__file__).read_bytes())
gate_binding=catalog(GATE);result=read(GATE/'result.json')
need(result['workspace_tests_per_toolchain']==313 and result['actual_live_tests_per_toolchain']==39,'Unexpected scope')
frozen=read(GATE/'source-inputs.json');source_map={Path(x['path']):x for x in frozen}
for row in frozen:
    need(sha(Path(row['path']))==row['sha256'] and sha(GATE/row['snapshot'])==row['sha256'],'Current source or archived input differs')
native=ROOT/'docs/phase-2/evidence/mysql-type-gate/run_20261115_214918_18929a67'
before={Path(x['path']):x for x in read(native/'source-inputs.json')}
unchanged_core=[];changed=[];new=[]
for row in frozen:
    path=Path(row['path'])
    if path.is_relative_to(ROOT/'crates'):
        previous=before.get(path)
        if previous and previous['sha256']!=row['sha256']:changed.append(str(path.relative_to(ROOT)))
        elif not previous:new.append(str(path.relative_to(ROOT)))
        if path.suffix=='.rs' and any(path.is_relative_to(ROOT/'crates'/package/'src') for package in ['seata_protocol','seata_transport','seata_tm','seata_rm']):
            need(previous and previous['sha256']==row['sha256'],'Control plane code changed');unchanged_core.append(str(path.relative_to(ROOT)))
lines=[dict(path=name,physical_lines=len((ROOT/name).read_text('utf-8').splitlines())) for name in new+changed if name.endswith('.rs')]
need(all(x['physical_lines']<=500 for x in lines),'New authored Rust file requires cohesion review')
save('production-scope.json',dict(changed=changed,added=new,unchanged_control_plane_sources=unchanged_core,authored_Rust_line_counts=lines))
controls=[]
for row in read(GATE/'controls.json'):
    source=Path(row['source']);directory=source.parents[5]
    need((directory/'Cargo.toml').exists(),'Could not locate isolated control root')
    count=0;mutated=[];extra_fixtures=[]
    for part in ['Cargo.toml','Cargo.lock','crates','tests/fixtures']:
        candidates=[directory/part] if (directory/part).is_file() else [p for p in (directory/part).rglob('*') if p.is_file()]
        for p in candidates:
            original=ROOT/p.relative_to(directory);parent=source_map.get(original)
            if not parent:
                need(p.is_relative_to(directory/'tests/fixtures') and original.read_bytes()==p.read_bytes(),'Unbound code input: '+str(p))
                extra_fixtures.append(dict(path=str(p),sha256=sha(p),scope='unconsumed extra fixture copied by isolation helper; not parent-frozen'))
                continue
            if sha(p)!=parent['sha256']:mutated.append(str(p))
            count+=1
    need(mutated==[str(source)],'Control has undeclared file mutation')
    controls.append(dict(name=row['name'],parent_frozen_inputs_checked=count,extra_unconsumed_fixtures=extra_fixtures,only_declared_file_changed=True))
save('pure-control-input-audit.json',controls)
children=[]
for child in read(GATE/'live-children.json'):children.append(catalog(Path(child['path'])))
save('live-catalog-audit.json',children)
peer=Path(read(GATE/'document-peer.json')['path']);catalog(peer)
for name in ['java-source-samples','java-compare','java-reencode','java-second-compare']:
    origins=read(peer/(name+'.origin-binding.json'));need(origins['all_file_origins_bound'],'Unbound positive JVM')
save('document-peer-audit.json',dict(packet=catalog(peer),runtime_matches=read(peer/'result.json')['second_Java_runtime_MATCH']))
# 被故意破坏的负向JVM在抛断言后，包装器没有复制class-load日志；独立从原执行目录补存并校验。
child=GATE/'live-control/child';owned=read(child/'owned-runtime.json');work=Path(owned['runtime'])/'prepared-peer'
negative_artifact=read(child/'test-artifact.json')
need(sha(Path(negative_artifact['path']))==negative_artifact['sha256']==sha(child/negative_artifact['snapshot']),'Negative executed Rust artifact differs')
sys.path.insert(0,str(ROOT/'tools/formal-undo-format'))
from runtime_binding import RuntimeBinding
binding=RuntimeBinding(ROOT/'docs/phase-2/evidence/undo-java-build/run_20261115_214102_f1bb6c34',ROOT.parent/'seata-java-phase09-oracle')
for row in read(child/'prepared-peer/peer-classes.json'):
    path=Path(row['path']);archived=child/'prepared-peer'/row['snapshot'];need(sha(path)==row['sha256']==sha(archived),'Negative peer class differs')
    binding.files[path.resolve()]=dict(sha256=row['sha256'],archive=str(archived),role='peer_compiled_class')
binding.roots.append((work/'classes').resolve());binding.verify()
loaded=work/'actual-JDBC-Rust-comparison.loaded.log';origins=binding.loaded(loaded,None,True);binding.verify()
(OUT/'negative-java-loaded.log').write_bytes(loaded.read_bytes());save('negative-java-origin-binding.json',origins)
save('result.json',dict(status='READ_ONLY_AUDIT_PASSED',gate=gate_binding,source_inputs_verified=len(frozen),
    sealed_live_children_verified=len(children),pure_controls=len(controls),negative_JVM_loaded_origins_now_bound=True,
    unchanged_control_plane_source_files=len(unchanged_core),Java_source_case_parity_complete=False,WP05_complete=False,
    SQL_Undo_atomicity_verified=False,public_COMMIT_available=False))
members=[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file()]
save('catalog.json',dict(members=members));print(json.dumps(dict(path=str(OUT),catalog_sha256=sha(OUT/'catalog.json'),members=len(members))))
