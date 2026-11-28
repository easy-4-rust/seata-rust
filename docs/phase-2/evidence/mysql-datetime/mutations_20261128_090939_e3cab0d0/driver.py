from pathlib import Path
import subprocess,os,json,datetime,uuid,hashlib,zipfile,re
ROOT=Path('E:/workspaces/workspace-github-easy-4-rust/seata-rust-phase09-validation');OUT=ROOT/'docs/phase-2/evidence/mysql-datetime'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);OUT.mkdir();print('DATETIME_MUTATIONS',OUT,flush=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,x):p.write_bytes((json.dumps(x,ensure_ascii=False,indent=2)+'\n').encode())
rows=[]
with zipfile.ZipFile(OUT/'source-inputs.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock']):name=p.relative_to(ROOT).as_posix();z.writestr(name,p.read_bytes());rows.append(dict(path=name,sha256=sha(p)))
save(OUT/'source-inputs.json',rows);commands=[]
for version in ['1.99','1.85']:
 env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_TARGET_DIR=str(ROOT.parent/('.seata-target-nodebug' if version=='1.99' else '.seata-target-nodebug-185')),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
 for mode in ['utc-profile','negative-epoch','microsecond','java-key']:
  path=ROOT/({'utc-profile':'crates/seata_at/src/undo/parser/prepared_format.rs','negative-epoch':'crates/seata_at/src/undo/parser/restored_format.rs','microsecond':'crates/seata_datasource/src/struct/local_date_time.rs','java-key':'crates/seata_datasource/src/struct/local_date_time.rs'}[mode]);original=path.read_bytes();text=original.decode();label=version+'-'+mode
  if mode=='utc-profile':changed=re.sub(r'                if !profile.utc_temporal\(\) \{.*?\n                \}', '',text,count=1,flags=re.S);name='utc_undo_preserves_negative_epoch_and_microseconds_and_refuses_old_profiles'
  elif mode=='negative-epoch':changed=text.replace('millis.div_euclid(1000)','millis / 1000');name='utc_undo_preserves_negative_epoch_and_microseconds_and_refuses_old_profiles'
  elif mode=='microsecond':changed=text.replace('            || nanosecond % 1000 != 0\n','');name='calendar_precision_and_profile_boundaries_are_not_guessed'
  else:changed=text.replace('if self.second != 0 || self.nanosecond != 0 {','if true {');name='datetime_primary_key_uses_java_seconds_and_fraction_rules'
  target=['--test','datetime_column',name]
  assert changed!=text;(OUT/(label+'.rs')).write_bytes(changed.encode());argv=['cargo','+'+version+'.0','test','-p','seata-db-mysql',*target,'--offline','--locked']
  try:
   path.write_bytes(changed.encode());r=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=180,creationflags=subprocess.CREATE_NO_WINDOW);(OUT/(label+'.stdout')).write_bytes(r.stdout);(OUT/(label+'.stderr')).write_bytes(r.stderr);assert r.returncode==101 and b'test result: FAILED. 0 passed; 1 failed;' in r.stdout,(label,r.stdout[-1200:],r.stderr[-1200:]);commands.append(dict(label=label,argv=argv,exit_code=r.returncode,source=path.relative_to(ROOT).as_posix(),original_sha256=hashlib.sha256(original).hexdigest(),mutated_sha256=hashlib.sha256(changed.encode()).hexdigest()));save(OUT/'commands.json',commands);print('Rejected',label,flush=True)
  finally:path.write_bytes(original)
for row in rows:assert sha(ROOT/row['path'])==row['sha256']
save(OUT/'result.json',dict(status='DATETIME_SOURCE_ASSERTION_COUNTEREXAMPLES_REJECTED',actual_target_failures=len(commands),whole_plan_complete=False));(OUT/'driver.py').write_bytes(Path(__file__).read_bytes());save(OUT/'catalog.json',[dict(path=p.relative_to(OUT).as_posix(),sha256=sha(p)) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='catalog.json']);print('CATALOG',sha(OUT/'catalog.json'),flush=True)
