# Owned FILE TC + loopback proxy; each run gets new runtime/evidence directories.
param(
    [string]$JavaSource='E:\workspaces\workspace-github-easy-4-rust\seata-java-phase09-oracle',
    [string]$RuntimeBase='D:\codex-builds\seata-phase09-response-loss'
)
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path "$PSScriptRoot/../../..").Path
if ($RuntimeBase -notmatch '^[DE]:\\') {throw 'Runtime must stay on D: or E:'}
$expectedJava='cba73e180fd8896a99df6415b859d275122feaa7'
if ((& git -C $JavaSource rev-parse HEAD) -ne $expectedJava -or $LASTEXITCODE -ne 0) {throw 'Java baseline mismatch'}
$jar=Join-Path $JavaSource 'server/target/seata-server.jar'
$jarHash=(Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash
if ($jarHash -ne 'AEF2E8386C97DEB78B45C6AD662AD601447DEF6F5C23537B7D5B1A314654C5D5') {throw 'Unverified Java TC artifact'}
if (Get-NetTCPConnection -LocalPort 28091,28092 -State Listen -ErrorAction SilentlyContinue) {throw 'Isolated test port already in use'}
$pythonExe='C:\Users\hnxyh\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
if (!(Test-Path -LiteralPath $pythonExe)) {throw 'Existing Python runtime not found; no installation attempted'}
$runId=(Get-Date -Format yyyyMMdd_HHmmss)+'_'+[guid]::NewGuid().ToString('N').Substring(0,8)
$evidenceRoot=Join-Path $repoRoot 'docs/phase-0/evidence/local-phase09-response-loss'
$evidenceDir=Join-Path $evidenceRoot "run_$runId"
$runtimeRoot=Join-Path $RuntimeBase $runId
New-Item -ItemType Directory -Path $evidenceDir,$runtimeRoot | Out-Null
$env:CARGO_TARGET_DIR='D:\cargo-target\seata-phase09'
$env:TEMP='D:\Temp';$env:TMP='D:\Temp';$env:PYTHONDONTWRITEBYTECODE='1'
$env:SEATA_TC_WORK_DIR=$runtimeRoot.Replace('\','/')
$env:SEATA_TC_HOST='127.0.0.1';$env:SEATA_TC_PORT='28092'
$env:SEATA_IT_TX_GROUP='default_tx_group';$env:SEATA_IT_HOLD_MS='0'
& cargo build --manifest-path "$repoRoot/tools/java-tc-integration/rust-client/Cargo.toml" *> "$evidenceDir/build.txt"
if ($LASTEXITCODE -ne 0) {throw 'Probe build failed'}
$probe=Join-Path $env:CARGO_TARGET_DIR 'debug/seata-java-tc-it.exe'
$probeHash=(Get-FileHash -LiteralPath $probe -Algorithm SHA256).Hash
$configText=(Get-Content "$repoRoot/tools/java-tc-integration/config/application.yml" -Raw).Replace('18091','28091')
$configText | Set-Content "$runtimeRoot/application.yml"
Copy-Item "$runtimeRoot/application.yml" "$evidenceDir/application.yml"
$tcProcess=$null;$proxyProcess=$null;$succeeded=$false;$results=@()

function Stop-Owned([System.Diagnostics.Process]$process) {
    if ($process) {
        $process.Refresh()
        if (!$process.HasExited) {$process.Kill();if (!$process.WaitForExit(10000)) {throw 'Owned process did not exit'}}
    }
}
function Wait-Listener([System.Diagnostics.Process]$process,[int]$port,[int]$seconds) {
    $deadline=(Get-Date).AddSeconds($seconds)
    do {
        $process.Refresh();if ($process.HasExited) {throw "Owned process exited before port $port readiness"}
        $listener=Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
        if ($listener -and $listener.OwningProcess -eq $process.Id) {return}
        if ((Get-Date) -ge $deadline) {throw "Owned process readiness timed out on $port"}
        Start-Sleep -Milliseconds 150
    } while ($true)
}

try {
    $config='file:'+$runtimeRoot.Replace('\','/')+'/application.yml'
    $tcProcess=Start-Process -FilePath D:\Java\jdk-17.0.12\bin\java.exe -WindowStyle Hidden -PassThru `
        -WorkingDirectory $runtimeRoot -RedirectStandardOutput "$evidenceDir/tc-stdout.log" -RedirectStandardError "$evidenceDir/tc-stderr.log" `
        -ArgumentList @('-Djava.io.tmpdir=D:/Temp',('"-Duser.home={0}"' -f $runtimeRoot),('"-Dspring.config.location={0}"' -f $config),'-jar',('"{0}"' -f $jar),'-h','127.0.0.1')
    Wait-Listener $tcProcess 28091 90
    foreach ($case in @(
        @{name='branch-register';operation=11;response=12;callback=5},
        @{name='commit';operation=7;response=8;callback=3},
        @{name='rollback';operation=9;response=10;callback=5}
    )) {
        $caseDir=Join-Path $evidenceDir $case.name
        New-Item -ItemType Directory -Path $caseDir | Out-Null
        $env:SEATA_IT_APP_ID="orders-loss-$($case.name)-$runId"
        $env:SEATA_IT_RESOURCE_ID="mysql://orders-loss-$($case.name)-$runId"
        $eventFile=Join-Path $caseDir 'proxy-events.jsonl'
        $proxyPath=Join-Path $repoRoot 'tools/java-tc-integration/response_loss/loss_proxy.py'
        $proxyProcess=Start-Process -FilePath $pythonExe -WindowStyle Hidden -PassThru -WorkingDirectory $runtimeRoot `
            -RedirectStandardOutput "$caseDir/proxy-stdout.log" -RedirectStandardError "$caseDir/proxy-stderr.log" `
            -ArgumentList @(('"{0}"' -f $proxyPath),'--listen-port','28092','--upstream-port','28091','--drop-response-kind',([string]$case.response),'--events',('"{0}"' -f $eventFile))
        Wait-Listener $proxyProcess 28092 10
        & $probe "--lost-$($case.name)-response" *> "$caseDir/client.txt"
        $clientExit=$LASTEXITCODE
        if ($clientExit -ne 0) {throw "Client failed: $($case.name)"}
        if (!$proxyProcess.WaitForExit(10000)) {throw 'Proxy failed to drain owned connections'}
        $proxyProcess.Refresh()
        if ($proxyProcess.ExitCode -ne 0) {throw "Proxy failed: $($case.name)"}
        $events=@(Get-Content -LiteralPath $eventFile | ForEach-Object {$_ | ConvertFrom-Json})
        $summaries=@($events | Where-Object event -eq 'summary')
        $drops=@($events | Where-Object { $_.event -eq 'frame' -and $_.dropped })
        if ($summaries.Count -ne 1 -or !$summaries[0].succeeded -or $drops.Count -ne 1 -or $drops[0].kind -ne $case.response) {throw 'Missing exact proxy loss proof'}
        $targetXid=$drops[0].xid
        if ([string]::IsNullOrWhiteSpace($targetXid)) {throw 'No target XID in dropped-frame evidence'}
        $requests=@($events | Where-Object {$_.event -eq 'frame' -and $_.direction -eq 'c2s' -and $_.request_type -eq 0})
        $registrations=@($requests | Where-Object kind -eq 11)
        $originalMutations=@($requests | Where-Object {$_.xid -eq $targetXid -and $_.kind -eq $case.operation})
        $originalEnds=@($requests | Where-Object {$_.xid -eq $targetXid -and $_.kind -in 7,9})
        if ($registrations.Count -ne 1 -or $registrations[0].xid -ne $targetXid -or $originalMutations.Count -ne 1 -or $originalEnds.Count -ne 1) {throw 'Mutating request replay or missing target mutation'}
        $callbackRequests=@($events | Where-Object {$_.event -eq 'frame' -and $_.direction -eq 's2c' -and $_.request_type -eq 0 -and $_.kind -eq $case.callback})
        $callbackReplies=@($events | Where-Object {$_.event -eq 'frame' -and $_.direction -eq 'c2s' -and $_.request_type -eq 1 -and $_.kind -eq ($case.callback+1)})
        if ($callbackRequests.Count -ne 1 -or $callbackReplies.Count -ne 1) {throw 'Expected one actual phase-two callback and reply'}
        $branchId=$callbackRequests[0].branch_id
        if ($branchId -le 0 -or $callbackRequests[0].xid -ne $targetXid -or $callbackReplies[0].xid -ne $targetXid -or $callbackReplies[0].branch_id -ne $branchId -or $callbackReplies[0].request_id -ne $callbackRequests[0].request_id) {throw 'Callback identity/correlation mismatch'}
        if ($case.operation -eq 11 -and $drops[0].branch_id -ne $branchId) {throw 'Callback did not identify the actually registered branch'}
        $clientText=Get-Content "$caseDir/client.txt" -Raw
        if (!$clientText.Contains("UNKNOWN operation=$($case.operation) requestId=$($drops[0].request_id) xid=$targetXid") -or !$clientText.Contains("PASS response-loss operation=$($case.operation) xid=$targetXid branchId=$branchId") -or !$clientText.Contains('lockable=false expected=false') -or !$clientText.Contains('lockable=true expected=true')) {throw 'Client uncertainty/lock/callback checks missing'}
        $final=[regex]::Match($clientText,'PASS response-loss .*observedStatus=(\d+) confirmedEnd=(true|false)')
        if (!$final.Success) {throw 'No final client observation'}
        $observed=[int]$final.Groups[1].Value
        if ($case.operation -ne 11 -and $observed -eq 15 -and (!$clientText.Contains("UNKNOWN_RETAINED operation=$($case.operation)") -or $final.Groups[2].Value -ne 'false')) {throw 'Finished was incorrectly promoted into a definite success'}
        if ($case.operation -eq 11 -and (!$clientText.Contains('initialBranchId=None') -or !$clientText.Contains('learnedFromCallback=true') -or !$clientText.Contains('registrationReconciled=true'))) {throw 'Lost registration identity was not learned from the callback'}
        $verb=if ($case.callback -eq 3) {'Commit'} else {'Rollback'}
        $needle="$verb branch transaction successfully, xid = $targetXid branchId = $branchId"
        $deadline=(Get-Date).AddSeconds(5)
        while (!(Get-Content "$evidenceDir/tc-stdout.log" -Raw).Contains($needle)) {
            if ((Get-Date) -ge $deadline) {throw 'Missing actual TC branch completion log'}
            Start-Sleep -Milliseconds 100
        }
        $results+=[ordered]@{scenario=$case.name;operation=$case.operation;xid=$targetXid;branch_id=$branchId;client_exit=$clientExit;proxy_exit=$proxyProcess.ExitCode;drop_count=1;mutating_target_attempts=1;branch_registrations=1;phase_two_actions=1;observed_status=$observed;confirmed_end=($final.Groups[2].Value -eq 'true');registration_reconciled=($case.operation -eq 11);unknown_end_retained=($case.operation -ne 11 -and $observed -eq 15 -and $final.Groups[2].Value -eq 'false');lock_released=$true;callback_request_id=$callbackRequests[0].request_id;proxy_pid=$proxyProcess.Id}
        $proxyProcess=$null
        Write-Output "PASS actual response loss scenario=$($case.name) xid=$targetXid branchId=$branchId status=$observed"
    }
    $succeeded=$true
} finally {
    $cleanupErrors=@()
    foreach ($ownedProcess in @($proxyProcess,$tcProcess)) {
        try {Stop-Owned $ownedProcess} catch {$cleanupErrors+=$_.Exception.Message}
    }
    $portsReleased=!(Get-NetTCPConnection -LocalPort 28091,28092 -State Listen -ErrorAction SilentlyContinue)
    $finalSuccess=$succeeded -and $portsReleased -and $cleanupErrors.Count -eq 0
    [ordered]@{executed_at=(Get-Date).ToString('o');succeeded=$finalSuccess;java_source=$expectedJava;server_jar_sha256=$jarHash;rust_probe_sha256=$probeHash;tc_pid=if ($tcProcess) {$tcProcess.Id} else {$null};port_released=$portsReleased;cleanup_errors=$cleanupErrors;scenarios=$results;scope='One complete successful response discarded externally on live V1 channels; simulated RM only, no DB/Undo Log; Finished alone retains uncertainty'} | ConvertTo-Json -Depth 6 | Set-Content "$evidenceDir/result.json"
    [ordered]@{evidence_directory=$evidenceDir;runtime_directory=$runtimeRoot;succeeded=$finalSuccess} | ConvertTo-Json | Set-Content "$evidenceRoot/latest_run.json"
    if ($cleanupErrors.Count) {throw ('Owned-process cleanup failed: '+($cleanupErrors -join '; '))}
    if (!$portsReleased) {throw 'Owned test ports not released'}
}
Get-Content "$evidenceDir/result.json"
