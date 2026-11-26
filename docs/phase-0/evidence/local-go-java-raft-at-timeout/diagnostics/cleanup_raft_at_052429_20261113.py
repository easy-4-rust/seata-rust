import importlib.util,json
from pathlib import Path
root=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');out=root/'docs/phase-0/evidence/local-go-java-raft-at-timeout/run_20261113_052429_66a3383e';runtime=Path('D:/codex-builds/rat_20261113_052429_66a3383e')
spec=importlib.util.spec_from_file_location('owned_cleanup_support',root/'tools/java-tc-integration/backend_recovery/run.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
original=json.loads((out/'container.json').read_bytes());identifier=original['Id'];item=json.loads(h.command(['docker','inspect',identifier]).stdout)[0]
mount=next(m for m in item['Mounts'] if m['Destination']=='/var/lib/mysql')
assert item['Id']==identifier and item['Image']==h.IMAGES['db'] and item['Config']['Labels']['seata-raft-at']==runtime.name
assert mount['Source'].replace('\\','/').lower()==(runtime/'mysql').as_posix().lower()
h.command(['docker','stop','-t','10',identifier],timeout=30);item=json.loads(h.command(['docker','inspect',identifier]).stdout)[0];assert item['State']['Status']=='exited'
ports=[28291,28293,28295,28101,28102,28103,28092,28093];assert not any(h.port_open(p) for p in ports)
receipt=out/'manual-cleanup-verification.json';value=dict(container=identifier,state='exited',owned_mount=mount['Source'],ports_released=ports,reason='Final source-audit loop shadowed MySQL data-path variable; bounded cleanup confirmed exact container image/label/mount before stopping.')
with receipt.open('x',encoding='utf-8') as stream:json.dump(value,stream,indent=2)
print(json.dumps(dict(succeeded=True,container_state='exited')))
