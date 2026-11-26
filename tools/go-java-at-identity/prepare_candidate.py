"""Independent0007 candidate, derived from the certified0006 cohort without editing it."""
from pathlib import Path
import difflib,hashlib,importlib.util,json,subprocess
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;PARENT=Path('D:/codex-builds/sif_20261112_232026_e565657b');DEST=ROOT/'compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498';PATCH=DEST/'0007-explicit-java-at-resource-and-lock-identity.patch';MANIFEST=DEST/'at_identity_manifest.json'
def sha(data):return hashlib.sha256(data).hexdigest()
def files():
 paths=['pkg/datasource/sql/driver.go','pkg/datasource/sql/db.go','pkg/datasource/sql/tx.go'];content={path:(PARENT/path).read_text(encoding='utf-8') for path in paths}
 path=paths[0];needle='type seataDriver struct {';assert content[path].count(needle)==1;content[path]=content[path].replace(needle,needle+'\n javaATResourceID string // 显式入口固定的Java资源身份，默认为空。\n javaATResourceCreated func(*DBResource) // 注册失败时向显式入口交还本次资源所有权。')
 needle='options := []dbOption{\n\t\twithResourceID(parseResourceID(dataSourceName)),';assert needle in content[path];content[path]=content[path].replace(needle,'resourceID:=parseResourceID(dataSourceName);if d.javaATResourceID!=""{resourceID=d.javaATResourceID}\n options := []dbOption{\n withResourceID(resourceID),\n withJavaATIdentity(d.javaATResourceID!=""),')
 needle='if err = datasource.GetDataSourceManager(d.branchType).RegisterResource(res); err != nil {';assert content[path].count(needle)==1;content[path]=content[path].replace(needle,'if d.javaATResourceCreated!=nil{d.javaATResourceCreated(res)}\n '+needle)
 path=paths[1];needle='resourceID string';assert content[path].count(needle)==1;content[path]=content[path].replace(needle,needle+'\n javaATIdentity bool // 创建时固定，禁止注册后迁移身份。');content[path]+='\nfunc withJavaATIdentity(enabled bool) dbOption{return func(db *DBResource){db.javaATIdentity=enabled}}\n'
 path=paths[2];needle='request.LockKeys = lockKey';assert content[path].count(needle)==1;content[path]=content[path].replace(needle,'if tx.conn!=nil&&tx.conn.res!=nil&&tx.conn.res.javaATIdentity {var err error;lockKey,err=javaATLockKeys(ctx,tx.conn.res);if err!=nil{return err};if lockKey==""{return nil}}\n '+needle)
 for name in ['java_at_mysql.go','java_at_lock_keys.go','java_at_key_order.go']:content['pkg/datasource/sql/'+name]=(HERE/name).read_text(encoding='utf-8')
 return {path:subprocess.run(['gofmt'],input=text.encode(),capture_output=True,check=True).stdout for path,text in content.items()}
def run():
 spec=importlib.util.spec_from_file_location('at_identity_parent_audit',ROOT/'tools/go-java-v1-inflight/audit.py');a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a);a.run(PARENT);content=files();rows=[];parts=[]
 for path,new in content.items():
  old=(PARENT/path).read_bytes() if (PARENT/path).exists() else b'';parts.append(f'diff --git a/{path} b/{path}\n')
  if not old:parts.append('new file mode 100644\n')
  parts.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),new.decode().splitlines(keepends=True),fromfile='a/'+path if old else '/dev/null',tofile='b/'+path));rows.append(dict(path=path,parent_sha256=sha(old) if old else None,patched_sha256=sha(new)))
 raw=''.join(parts).encode();manifest=dict(go_pin='82570d905919c1c66bb025a3a5979f25390de498',patch=PATCH.name,patch_sha256=sha(raw),requires={p.name:sha(p.read_bytes()) for p in sorted(DEST.glob('000[1-6]-*.patch'))},files=rows,scope='Explicit Java AT MySQL resource/lock identity, new entry only; no default migration or Undo-format interchange.')
 for path,data in [(PATCH,raw),(MANIFEST,(json.dumps(manifest,indent=2)+'\n').encode())]:
  if path.exists():assert path.read_bytes()==data,('Refusing overwrite',path)
  else:path.write_bytes(data)
 print(json.dumps(dict(patch_sha256=sha(raw),production_files=len(content))))
if __name__=='__main__':run()
