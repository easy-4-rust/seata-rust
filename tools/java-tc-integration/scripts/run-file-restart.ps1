# Isolated FILE process-crash tests. Every run gets fresh directories; no existing data is reset.
param(
    [string]$JavaSource = 'E:\workspaces\workspace-github-easy-4-rust\seata-java-phase09-oracle',
    [string]$RuntimeBase = 'D:\codex-builds\seata-phase09-restart'
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path "$PSScriptRoot/../../..").Path
if ($RuntimeBase -notmatch '^[DE]:\\') { throw 'Runtime files must be on D: or E:' }
$expectedJava = 'cba73e180fd8896a99df6415b859d275122feaa7'
if ((& git -C $JavaSource rev-parse HEAD) -ne $expectedJava -or $LASTEXITCODE -ne 0) { throw 'Java source mismatch' }
$jar = Join-Path $JavaSource 'server/target/seata-server.jar'
$jarHash = (Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash
if ($jarHash -ne 'AEF2E8386C97DEB78B45C6AD662AD601447DEF6F5C23537B7D5B1A314654C5D5') { throw 'Unverified server artifact' }
if (Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue) { throw 'Test port already in use' }
$runId = (Get-Date -Format yyyyMMdd_HHmmss) + '_' + [guid]::NewGuid().ToString('N').Substring(0,8)
$evidenceRoot = Join-Path $repoRoot 'docs/phase-0/evidence/local-phase09-restart'
$evidenceDir = Join-Path $evidenceRoot "run_$runId"
$runtimeRoot = Join-Path $RuntimeBase $runId
New-Item -ItemType Directory -Path $evidenceDir,$runtimeRoot | Out-Null
$env:CARGO_TARGET_DIR = 'D:\cargo-target\seata-phase09'
$env:TEMP = 'D:\Temp'
$env:TMP = 'D:\Temp'
$env:SEATA_TC_HOST = '127.0.0.1'
$env:SEATA_TC_PORT = '28091'
$env:SEATA_IT_RESOURCE_ID = 'mysql://orders'
$env:SEATA_IT_APP_ID = 'orders-rust'
$env:SEATA_IT_TX_GROUP = 'default_tx_group'
$env:SEATA_IT_HOLD_MS = '0'
& cargo build --manifest-path "$repoRoot/tools/java-tc-integration/rust-client/Cargo.toml" *> "$evidenceDir/build.txt"
if ($LASTEXITCODE -ne 0) { throw 'Probe build failed' }
$probe = Join-Path $env:CARGO_TARGET_DIR 'debug/seata-java-tc-it.exe'
$script:activeTc = $null
$results = @()

function Stop-OwnedTc {
    if ($script:activeTc) {
        $script:activeTc.Refresh()
        if (!$script:activeTc.HasExited) {
            $script:activeTc.Kill()
            if (!$script:activeTc.WaitForExit(10000)) { throw 'Owned TC did not exit' }
        }
    }
    if (Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue) { throw 'Test port not released' }
}

function Start-OwnedTc([string]$runtimeDir,[string]$logPrefix) {
    $env:SEATA_TC_WORK_DIR = $runtimeDir.Replace('\','/')
    $config = 'file:' + $runtimeDir.Replace('\','/') + '/application.yml'
    $script:activeTc = Start-Process -FilePath 'D:\Java\jdk-17.0.12\bin\java.exe' -WindowStyle Hidden -PassThru `
        -WorkingDirectory $runtimeDir -RedirectStandardOutput "$logPrefix-stdout.log" -RedirectStandardError "$logPrefix-stderr.log" `
        -ArgumentList @('-Djava.io.tmpdir=D:/Temp',('"-Duser.home={0}"' -f $runtimeDir),('"-Dspring.config.location={0}"' -f $config),'-jar',('"{0}"' -f $jar),'-h','127.0.0.1')
    $deadline = (Get-Date).AddSeconds(90)
    do {
        $script:activeTc.Refresh()
        if ($script:activeTc.HasExited) { throw "TC exited before readiness: $logPrefix" }
        $listener = Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue
        if ($listener -and $listener.OwningProcess -eq $script:activeTc.Id) { return $script:activeTc }
        if ((Get-Date) -ge $deadline) { throw 'TC readiness timed out' }
        Start-Sleep -Milliseconds 300
    } while ($true)
}

$succeeded = $false
try {
    foreach ($scenario in @('begin','branch')) {
        $runtimeDir = Join-Path $runtimeRoot $scenario
        $caseDir = Join-Path $evidenceDir $scenario
        New-Item -ItemType Directory -Path $runtimeDir,$caseDir | Out-Null
        $configText = (Get-Content "$repoRoot/tools/java-tc-integration/config/application.yml" -Raw).Replace('18091','28091')
        $configText | Set-Content "$runtimeDir/application.yml"
        Copy-Item "$runtimeDir/application.yml" "$caseDir/application.yml"
        $env:SEATA_IT_RESTART_RECORD = Join-Path $runtimeDir 'restart_record.txt'
        $before = Start-OwnedTc $runtimeDir "$caseDir/before"
        $beforePid = $before.Id
        & $probe "--prepare-restart-$scenario" *> "$caseDir/prepare.txt"
        $prepareExit = $LASTEXITCODE
        if ($prepareExit -ne 0) { throw "Prepare failed: $scenario" }
        Copy-Item -LiteralPath $env:SEATA_IT_RESTART_RECORD -Destination "$caseDir/restart_record.txt"
        $store = Join-Path $runtimeDir 'data/sessionStore/28091/root.data'
        if (!(Test-Path -LiteralPath $store) -or (Get-Item -LiteralPath $store).Length -eq 0) { throw 'No FILE transaction data found' }
        Stop-OwnedTc
        $diskBefore = Get-ChildItem -LiteralPath (Split-Path $store) -File | ForEach-Object {
            [ordered]@{name=$_.Name;length=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
        }
        $diskBefore | ConvertTo-Json -Depth 4 | Set-Content "$caseDir/disk-after-crash.json"
        $after = Start-OwnedTc $runtimeDir "$caseDir/after"
        if ($after.Id -eq $beforePid) { throw 'Cannot demonstrate a distinct TC process' }
        & $probe --resume-restart *> "$caseDir/resume.txt"
        $resumeExit = $LASTEXITCODE
        if ($resumeExit -ne 0) { throw "Resume failed: $scenario" }
        # Allow the pinned TC's batched evidence logger to drain before forced cleanup.
        if ($scenario -eq 'branch') {
            $fields = Get-Content -LiteralPath $env:SEATA_IT_RESTART_RECORD
            $needle = "Rollback branch transaction successfully, xid = $($fields[1]) branchId = $($fields[2])"
            $deadline = (Get-Date).AddSeconds(5)
            while (!(Get-Content "$caseDir/after-stdout.log" -Raw).Contains($needle)) {
                if ((Get-Date) -ge $deadline) { throw 'Missing recovered-branch completion evidence' }
                Start-Sleep -Milliseconds 100
            }
        }
        Stop-OwnedTc
        $results += [ordered]@{scenario=$scenario;prepare_exit=$prepareExit;resume_exit=$resumeExit;before_pid=$beforePid;after_pid=$after.Id;runtime=$runtimeDir;record_sha256=(Get-FileHash $env:SEATA_IT_RESTART_RECORD -Algorithm SHA256).Hash;file_bytes_after_crash=(Get-Content "$caseDir/disk-after-crash.json" -Raw | ConvertFrom-Json).length;port_released=$true}
        Write-Output "PASS FILE restart scenario=$scenario beforePid=$beforePid afterPid=$($after.Id)"
    }
    $succeeded = $true
} finally {
    Stop-OwnedTc
    [ordered]@{executed_at=(Get-Date).ToString('o');succeeded=$succeeded;java_source=$expectedJava;server_jar_sha256=$jarHash;forced_process_termination=$true;no_storage_reset=$true;scenarios=$results;scope='FILE normal Begin/Registered records, process restart only; no database or power-loss guarantee'} | ConvertTo-Json -Depth 6 | Set-Content "$evidenceDir/result.json"
    [ordered]@{evidence_directory=$evidenceDir;runtime_directory=$runtimeRoot;succeeded=$succeeded} | ConvertTo-Json | Set-Content "$evidenceRoot/latest_run.json"
}
Get-Content "$evidenceDir/result.json"
