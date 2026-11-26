"""Check class bytes against the sealed, already certified Java parent inventory."""
import hashlib,json
from pathlib import Path
PARENT_MANIFEST_SHA256='743a16b3541fe5b618f939628dfb2bf806f60c6bf0e4dc784e82341bca6fed8a'
def digest(data):return hashlib.sha256(data).hexdigest()
def verify(parent):
 parent=Path(parent);catalog=parent.parent/'manifest.json';raw=catalog.read_bytes()
 if digest(raw)!=PARENT_MANIFEST_SHA256:raise RuntimeError('Certified Java parent manifest identity changed')
 inventory=parent/'overlay-manifest.json';relative=inventory.relative_to(catalog.parent).as_posix();expected=next(row['sha256'] for row in json.loads(raw)['files'] if row['path']==relative)
 if digest(inventory.read_bytes())!=expected:raise RuntimeError('Certified Java overlay inventory changed')
 rows=json.loads(inventory.read_bytes())
 if len(rows)!=10:raise RuntimeError('Certified Java overlay class count differs')
 actual={p.relative_to(parent/'overlay').as_posix() for p in (parent/'overlay').rglob('*.class')}
 if actual!={r['path'] for r in rows}:raise RuntimeError('Unexpected Java overlay class')
 for row in rows:
  if digest((parent/'overlay'/row['path']).read_bytes())!=row['sha256']:raise RuntimeError('Certified Java class bytes changed: '+row['path'])
 return rows
