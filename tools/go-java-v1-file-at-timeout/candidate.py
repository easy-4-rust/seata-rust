"""Authenticate the sealed 19-class Java candidate and actual runtime class sources."""
import hashlib,json,re
from pathlib import Path
PARENT_SHA='90f7cc98a194ebb652ff0a1fe725b4e01c16daf08479309549ba3483118263a1'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def verify(parent):
 parent=Path(parent);catalog=parent.parent/'manifest.json'
 if sha(catalog)!=PARENT_SHA:raise RuntimeError('Certified Java retention catalog changed')
 rows=json.loads(catalog.read_bytes())['files'];inventory=parent/'overlay-manifest.json'
 def checked(path):
  expected=next(row['sha256'] for row in rows if row['path']==path.relative_to(catalog.parent).as_posix())
  if sha(path)!=expected:raise RuntimeError('Certified Java candidate artifact changed: '+str(path))
 checked(inventory);values=json.loads(inventory.read_bytes())
 if len(values)!=19 or {p.relative_to(parent/'overlay').as_posix() for p in (parent/'overlay').rglob('*.class')}!={row['path'] for row in values}:raise RuntimeError('Certified Java candidate class inventory differs')
 for row in values:
  path=parent/'overlay'/row['path'];checked(path)
  if sha(path)!=row['sha256']:raise RuntimeError('Certified candidate class differs')
 return values
def class_sources(log,overlay):
 text=Path(log).read_text(encoding='utf-8',errors='replace');expected='file:/'+Path(overlay).as_posix().rstrip('/')+'/'
 result=[]
 for name in ['org.apache.seata.server.session.SessionHolder','org.apache.seata.server.coordinator.DefaultCoordinator','org.apache.seata.server.storage.file.session.FileSessionManager']:
  actual=re.findall(r'\[class,load\s*\]\s+'+re.escape(name)+r'\s+source:\s+(\S+)',text)
  if actual!=[expected]:raise RuntimeError('Actual FILE recovery candidate origin differs: '+name)
  result.append(dict(type=name,source=expected))
 return result
