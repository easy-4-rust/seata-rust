# 仅在本轮测试进程结束后执行；核对自有中继、容器和隔离引擎身份。
$ErrorActionPreference='Stop'
$e='docs/phase-2/evidence/mysql-recovery-current-schema'
$listener=Get-NetTCPConnection -LocalPort 43318 -State Listen
if($listener.OwningProcess -ne 44340){throw 'Unexpected relay owner'}
$relay=Get-CimInstance Win32_Process -Filter 'ProcessId=44340'
if($relay.CommandLine -notlike '*fastjson-undo-native/run_20261120_owned_wsl_mysql/wsl_stdio_relay.py*'){throw 'Unexpected relay command'}
Stop-Process -Id 44340
'Owned relay closed' | Set-Content "$e/cleanup.log"
wsl -d Ubuntu-20.04 -- bash /mnt/d/codex-builds/cleanup-recovery-current-schema.sh >> "$e/cleanup.log"
if($LASTEXITCODE){exit $LASTEXITCODE}
Get-Content "$e/cleanup.log"
