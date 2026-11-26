"""Run exact current-source roles; Java's three owned ports are serialized."""
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import threading
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('callback_owned_live',ROOT/'tools/formal-callback-ownership/live.py');live=importlib.util.module_from_spec(spec);spec.loader.exec_module(live)
java_lock=threading.Lock()
def run(rust,role):
    if role=='java':
        with java_lock:return live.run(rust,None,False,'unused',java=True)
    return live.run(rust,None,False,'unused',fault='all' if role=='faults' else None)
if __name__=='__main__':
    manifest=dict(profiles={rust:{} for rust in ['1.99','1.85']},red=dict(path='docs/phase-2/evidence/mysql-callback-ownership/run_20261117_132256_6e854b0d',catalog_sha256='04550eb7788395618be3454ff725df97c85bc6021dde7475c25cab15647f4cd4'))
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        pending={executor.submit(run,rust,role):(rust,role) for role in ['native','faults','java'] for rust in ['1.99','1.85']}
        for future in concurrent.futures.as_completed(pending):
            rust,role=pending[future];result=future.result();manifest['profiles'][rust][role]=dict(path=Path(result['run']).relative_to(ROOT).as_posix(),catalog_sha256=result['catalog_sha256'])
    output=HERE/'qualification-inputs-20261117.json';output.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(str(output),flush=True)
