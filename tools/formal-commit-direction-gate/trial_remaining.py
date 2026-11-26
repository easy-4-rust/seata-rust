import concurrent.futures,importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('commit_direction_live_trial',ROOT/'tools/formal-commit-direction/live.py');live=importlib.util.module_from_spec(spec);spec.loader.exec_module(live)
jobs={'drop_ok':dict(commit_fault='drop_ok'),'delay_ok':dict(commit_fault='delay_ok'),'block_terminal':dict(commit_fault='block_terminal'),'java':dict(java=True)}
values={}
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    pending={pool.submit(live.run,'1.99',**args):name for name,args in jobs.items()}
    for future in concurrent.futures.as_completed(pending):
        name=pending[future];values[name]=future.result();print('TRIAL_ROLE_COMPLETE '+name,flush=True)
with (Path(__file__).parent/'trial-inputs-20261117.json').open('x',encoding='utf-8') as f:json.dump(values,f,indent=2);f.write('\n')
