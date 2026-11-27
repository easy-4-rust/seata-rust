"""Real API counterexamples; restore exact production bytes after every mutation."""
import datetime,json,os,subprocess,uuid
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run():
    out=ROOT/'docs/phase-2/evidence/structured-logs'/('mutations_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]);out.mkdir(parents=True)
    print(str(out),flush=True)
    rows=[]
    variants=[
        ('pre_format_schema','structured_log_schema.rs',b'meta.fields().len() == names.len()',b'true','unexpected_fields_are_rejected_before_debug_formatting','unexpected field was formatted before schema rejection'),
        ('nonblocking_queue','structured_log_subscriber.rs',b'self.sender.try_send(bytes)',b'{ if self.sender.capacity()==0 {self.sender.blocking_send(bytes.clone()).map_err(|e| mpsc::error::TrySendError::Closed(e.0))}else{self.sender.try_send(bytes)} }','concurrent_full_queue_never_waits_for_consumer','log producer waited for full queue consumer'),
        ('encoded_byte_budget','log_record_buffer.rs',b'if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {',b'if false {','oversized_fields_and_json_escaping_drop_whole_records','oversize record escaped JSON budget'),
        ('resource_digest','structured_log_schema.rs',b'"resource_sha256" => {',b'"resource_sha256" => { true ||','invalid_values_and_messages_are_not_exported','invalid field value reached JSON queue'),
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
                value=subprocess.run(['python','-B','tools/formal-source-junit/structured_logs_rust.py','--rust',version,'--case',case,'--expect-failure',marker],cwd=ROOT,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=600)
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
