"""Read-only audit of every owned preparation attempt; no process or container mutation."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[2]
def read(p):return json.loads(p.read_bytes())
def main(output):
    env=os.environ|dict(DOCKER_CONFIG='D:/Temp/seata-docker-config',TEMP='D:/Temp',TMP='D:/Temp');rows=[];java=[]
    for p in sorted((ROOT/'docs/phase-2/evidence/mysql-commit-preconditions').glob('run_*/owned-runtime.json')):
        row=read(p);value=subprocess.run(['docker','inspect',row['container_id']],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30)
        if value.returncode==0 or b'no such object' not in value.stdout.lower():raise ValueError('Owned container absence not confirmed: '+row['container_id'])
        cleanup=read(p.parent/'cleanup.json');data=Path(cleanup['D_data_retained'])
        if not data.resolve().is_relative_to(Path('D:/codex-builds').resolve()) or not data.is_dir():raise ValueError('Retained owned data path differs')
        rows.append(dict(run=str(p.parent),runtime_reference_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),container_id=row['container_id'],owner=row['label_owner'],verified_absent=True,D_data_retained=str(data)))
        owned=p.parent/'java-tc/owned-processes.json'
        if owned.exists():
            processes=read(owned);runtime=Path(processes['tc_command'][next(i for i,a in enumerate(processes['tc_command']) if a.startswith('-Duser.home='))].split('=',1)[1]);java.append(dict(run=str(p.parent),pids=[processes['tc_pid']]+processes['proxy_pids'],runtime=str(runtime)))
    pids=sorted({pid for item in java for pid in item['pids']})
    command="$wanted=@("+','.join(map(str,pids))+"); @((Get-CimInstance Win32_Process | Where-Object { $wanted -contains $_.ProcessId }) | ForEach-Object { [pscustomobject]@{pid=$_.ProcessId;command=$_.CommandLine} }) | ConvertTo-Json -Compress"
    value=subprocess.run(['powershell','-NoProfile','-Command',command],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW,timeout=30)
    if value.returncode:raise ValueError('Owned Java PID audit failed')
    raw=value.stdout.decode('utf-8-sig').strip();active=json.loads(raw) if raw else [];active=active if isinstance(active,list) else [active]
    for item in java:
        still_owned=[r['pid'] for r in active if r['pid'] in item['pids'] and item['runtime'].replace('\\','/') in (r.get('command') or '').replace('\\','/')]
        if still_owned:raise ValueError('Owned Java process remains')
        item['verified_owned_processes_absent']=True
    result=dict(created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),owned_containers_verified_absent=len(rows),containers=rows,owned_java_runs_verified_stopped=len(java),java=java,production_gate_qualified=False)
    output.parent.mkdir(parents=True,exist_ok=True)
    with output.open('x',encoding='utf-8') as stream:json.dump(result,stream,ensure_ascii=False,indent=2);stream.write('\n')
    print(json.dumps(dict(path=str(output),containers=len(rows),java_runs=len(java),sha256=hashlib.sha256(output.read_bytes()).hexdigest())))
if __name__=='__main__':main(Path(sys.argv[1]))
