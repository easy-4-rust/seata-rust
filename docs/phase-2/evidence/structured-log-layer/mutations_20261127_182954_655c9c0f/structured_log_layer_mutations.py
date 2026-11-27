"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-2/evidence/structured-log-layer'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    rows=[]
    variants=[
        ('global_filter','structured_log_subscriber.rs',b'.with_filter(',b'.and_then(','all_seven_templates_and_application_output_keep_separate_admission','application event missing from sibling layer'),
        ('omit_forward','structured_log_layer.rs',b'self.subscriber.event(event);',b'let _ = event;','all_seven_templates_and_application_output_keep_separate_admission','composed layer lost Seata events'),
        ('duplicate_forward','structured_log_layer.rs',b'self.subscriber.event(event);',b'self.subscriber.event(event); self.subscriber.event(event);','all_seven_templates_and_application_output_keep_separate_admission','composed layer lost Seata events'),
    ]
    originals={name:(ROOT/'crates/seata_observability/src'/name).read_bytes() for _,name,_,_,_,_ in variants}
    try:
        for version in ['1.99','1.85']:
            for name,file,site,replacement,case,marker in variants:
                path=ROOT/'crates/seata_observability/src'/file;original=originals[file]
                site=site.replace(b'\n',b'\r\n') if b'\r\n' in original else site
                replacement=replacement.replace(b'\n',b'\r\n') if b'\r\n' in original else replacement
                need(original.count(site)==1,'Unique actual EOL production site missing')
                path.write_bytes(original.replace(site,replacement))
                value=subprocess.run(['python','-B','tools/formal-source-junit/structured_log_layer_rust.py','--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
                (out/(version+'-'+name+'.stdout')).write_bytes(value.stdout);(out/(version+'-'+name+'.stderr')).write_bytes(value.stderr)
                need(value.returncode==0,'Mutation did not produce the required behavioral failure: '+value.stderr.decode(errors='replace'))
                last=json.loads(value.stdout.splitlines()[-1]);folder=Path(last['evidence']);rows.append(dict(Rust=version,operation=name,case=case,marker=marker,evidence=folder.relative_to(ROOT).as_posix(),catalog_sha256=last['catalog_sha256']))
                save(out/'mutations.json',rows);path.write_bytes(original)
                print('Rejected real '+name+' mutation Rust '+version,flush=True)
    finally:
        for name,original in originals.items():(ROOT/'crates/seata_observability/src'/name).write_bytes(original)
    save(out/'restored.json',[dict(path=(ROOT/'crates/seata_observability/src'/name).relative_to(ROOT).as_posix(),sha256=sha(ROOT/'crates/seata_observability/src'/name),exact_original_restored=(ROOT/'crates/seata_observability/src'/name).read_bytes()==original) for name,original in originals.items()])
    (out/Path(__file__).name).write_bytes(Path(__file__).read_bytes())
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':run()
