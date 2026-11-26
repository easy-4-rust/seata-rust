'Exact current two-profile role execution; no latest-run inference.'
import concurrent.futures,importlib.util,json,threading
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
live=module('current_commit_check_live',ROOT/'tools/formal-commit-preconditions/live.py')
maintenance=module('stopped_owned_data',HERE/'compress_owned.py')
java_lock=threading.Lock();io_lock=threading.Lock();values=dict(profiles={'1.99':{},'1.85':{}},data_maintenance=[])
manifest=HERE/'qualification-inputs-20261117.json'
if manifest.exists():raise ValueError('Refuse replacing explicit manifest')
def execute(rust,name):
    args=dict(rust=rust,target_override={'native':'live_commit_preconditions','preparation':'live_first_phase_preparation','regression':'live_callback_ownership','java':None}[name],java=name=='java')
    if name=='java':
        with java_lock:receipt=live.run(**args)
    else:receipt=live.run(**args)
    packet=Path(receipt['run']);audit=maintenance.run(packet)
    with io_lock:
        values['profiles'][rust][name]=dict(path=packet.relative_to(ROOT).as_posix(),catalog_sha256=receipt['catalog_sha256']);values['data_maintenance'].append(audit)
    print('CURRENT_ROLE_COMPLETE '+rust+'/'+name,flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    futures=[pool.submit(execute,rust,name) for name in ['native','preparation','regression','java'] for rust in ['1.99','1.85']]
    for f in concurrent.futures.as_completed(futures):f.result()
with manifest.open('x',encoding='utf-8') as f:json.dump(values,f,indent=2);f.write('\n')
print('EXPLICIT_MANIFEST '+str(manifest),flush=True)
