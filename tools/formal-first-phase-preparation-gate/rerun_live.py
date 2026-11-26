"""Current-source roles with exact returned receipts; owned Java ports are serialized."""
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import threading
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('owned_preparation_live',ROOT/'tools/formal-first-phase-preparation/live.py');live=importlib.util.module_from_spec(spec);spec.loader.exec_module(live)
java_lock=threading.Lock()
def run(rust,role):
    if role=='java':
        with java_lock:return live.run(rust,java=True)
    if role=='wire':return live.run(rust,wire=True)
    if role=='regression':return live.run(rust,target_override='live_callback_ownership')
    if role.startswith('enrollment_'):return live.run(rust,enrollment_fault=role[len('enrollment_'):])
    return live.run(rust)
if __name__=='__main__':
    value=dict(profiles={rust:{} for rust in ['1.99','1.85']},red=dict(path='docs/phase-2/evidence/mysql-first-phase-preparation/run_20261117_152519_adc2267c',catalog_sha256='709d7c9d1a4c1c0e7a09a7c0a3584d3b10d944c1351ab5ecf8ff994753a93474'))
    roles=['native','regression','wire','enrollment_block_commit','enrollment_drop_ok','enrollment_delay_ok','java']
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        pending={executor.submit(run,rust,role):(rust,role) for role in roles for rust in ['1.99','1.85']}
        for future in concurrent.futures.as_completed(pending):
            rust,role=pending[future];r=future.result();value['profiles'][rust][role]=dict(path=Path(r['run']).relative_to(ROOT).as_posix(),catalog_sha256=r['catalog_sha256'])
    output=HERE/'qualification-inputs-20261117.json';output.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(str(output),flush=True)
