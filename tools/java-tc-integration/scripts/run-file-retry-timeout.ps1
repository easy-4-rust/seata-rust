# Diagnostic fixture restore: unchanged pinned TC JAR, fresh FILE store per status, owned processes only.
param(
    [string]$JavaSource='E:\workspaces\workspace-github-easy-4-rust\seata-java-phase09-oracle',
    [string]$JavaBuild='D:\codex-builds\seata-java-exact-overlay-20261112',
    [string]$RuntimeBase='D:\codex-builds\seata-file-retry-timeout',
    [string]$MavenSettings='D:\codex-builds\seata-phase09-live-20261111\maven-central-settings.xml'
)
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path "$PSScriptRoot/../../..").Path
if ($RuntimeBase -notmatch '^[DE]:\\') {throw 'Runtime must stay on D: or E:'}
if ($JavaBuild -notmatch '^[DE]:\\') {throw 'Java build must stay on D: or E:'}
$javaPin='cba73e180fd8896a99df6415b859d275122feaa7'
if ((& git -C $JavaSource rev-parse HEAD) -ne $javaPin -or $LASTEXITCODE -ne 0) {throw 'Java source pin mismatch'}
$jar=Join-Path $JavaSource 'server/target/seata-server.jar'
$jarHash=(Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash
if ($jarHash -ne 'AEF2E8386C97DEB78B45C6AD662AD601447DEF6F5C23537B7D5B1A314654C5D5') {throw 'Unverified TC JAR'}
if (Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue) {throw 'Test port already in use'}
$javaExe='D:\Java\jdk-17.0.12\bin\java.exe'
$testRoot=Join-Path $JavaBuild 'test-suite/test-new-version'
$reportPath=Join-Path $testRoot 'target/surefire-reports/TEST-org.apache.seata.server.session.FileRetryTimeoutProcessTest.xml'
foreach ($name in @('FileRetryTimeoutStoreProbe','FileRetryTimeoutProcessTest')) {
    $executed=Join-Path $testRoot "src/test/java/org/apache/seata/server/$name.java"
    if ((Get-FileHash -LiteralPath $executed).Hash -ne (Get-FileHash -LiteralPath "$repoRoot/tools/java-v1-golden/$name.java").Hash) {throw 'Executed helper differs from repository source'}
}
$corePaths=@(
    'server/src/main/java/org/apache/seata/server/storage/file/session/FileSessionManager.java',
    'server/src/main/java/org/apache/seata/server/storage/file/store/FileTransactionStoreManager.java',
    'server/src/main/java/org/apache/seata/server/session/GlobalSession.java',
    'server/src/main/java/org/apache/seata/server/session/BranchSession.java',
    'server/src/main/java/org/apache/seata/server/session/SessionHolder.java'
)
foreach ($relative in $corePaths) {
    $expected=& git -C $JavaSource rev-parse "${javaPin}:$relative"
    if ($LASTEXITCODE -ne 0) {throw 'Cannot read pinned production source blob'}
    # Spotless uses Windows CRLF in this build tree; normalize ONLY CRLF before comparing the Git blob.
    $sourceText=[System.IO.File]::ReadAllText((Join-Path $JavaBuild $relative)).Replace("`r`n","`n")
    $sourceBytes=[System.Text.Encoding]::UTF8.GetBytes($sourceText)
    $blobHeader=[System.Text.Encoding]::ASCII.GetBytes(('blob '+$sourceBytes.Length+[char]0))
    $sha1=[System.Security.Cryptography.SHA1]::Create()
    try {$actual=([System.BitConverter]::ToString($sha1.ComputeHash([byte[]]($blobHeader+$sourceBytes)))).Replace('-','').ToLower()} finally {$sha1.Dispose()}
    if ($expected -ne $actual) {throw "Java production source changed: $relative"}
}
$runId=(Get-Date -Format yyyyMMdd_HHmmss)+'_'+[guid]::NewGuid().ToString('N').Substring(0,8)
$evidenceRoot=Join-Path $repoRoot 'docs/phase-0/evidence/local-file-retry-timeout'
$evidenceDir=Join-Path $evidenceRoot "run_$runId"
$runtimeRoot=Join-Path $RuntimeBase $runId
New-Item -ItemType Directory -Path $evidenceDir,$runtimeRoot | Out-Null
$env:CARGO_TARGET_DIR='D:\cargo-target\seata-phase09'
$env:TEMP='D:\Temp';$env:TMP='D:\Temp';$env:LOCALAPPDATA='D:\Temp\seata-localappdata'
$env:JAVA_HOME='D:\Java\jdk-17.0.12';$env:MAVEN_OPTS='-Djava.io.tmpdir=D:/Temp'
# Recompile the two owned helper classes and rerun tests; never accept an old XML as current evidence.
foreach ($name in @('FileRetryTimeoutStoreProbe','FileRetryTimeoutProcessTest')) {
    $classFile=Join-Path $testRoot "target/test-classes/org/apache/seata/server/session/$name.class"
    if (Test-Path -LiteralPath $classFile) {Remove-Item -LiteralPath $classFile}
}
& 'D:/Java/maven/apache-maven-3.9.9/bin/mvn.cmd' -B -ntp -s $MavenSettings -f "$JavaBuild/pom.xml" -pl test-suite/test-new-version -am '-Dtest=FileRetryTimeoutProcessTest' '-Dsurefire.failIfNoSpecifiedTests=false' '-Djava.io.tmpdir=D:/Temp' test *> "$evidenceDir/java-process-tests.log"
if ($LASTEXITCODE -ne 0) {throw 'Fresh Java process tests failed; live diagnostic not started'}
$report=[xml](Get-Content -LiteralPath $reportPath -Raw)
if ([int]$report.testsuite.tests -ne 5 -or [int]$report.testsuite.failures -ne 0 -or [int]$report.testsuite.errors -ne 0 -or [int]$report.testsuite.skipped -ne 0) {throw 'Fresh five process tests must pass before live diagnostic'}
$classpath=($report.testsuite.properties.property | Where-Object name -eq 'surefire.test.class.path').value
if (!$classpath) {throw 'No tested Java classpath recorded'}
Copy-Item -LiteralPath $reportPath -Destination "$evidenceDir/java-process-report.xml"
foreach ($name in @('FileRetryTimeoutStoreProbe','FileRetryTimeoutProcessTest')) {
    if ((Get-FileHash -LiteralPath (Join-Path $testRoot "src/test/java/org/apache/seata/server/$name.java")).Hash -ne (Get-FileHash -LiteralPath "$repoRoot/tools/java-v1-golden/$name.java").Hash) {throw 'Build formatter changed helper; reconcile tested source before proceeding'}
}
$env:SEATA_TC_HOST='127.0.0.1';$env:SEATA_TC_PORT='28091';$env:SEATA_IT_HOLD_MS='0'
$env:SEATA_IT_APP_ID='timeout-file-probe';$env:SEATA_IT_TX_GROUP='default_tx_group';$env:SEATA_IT_RESOURCE_ID='mysql://timeout-file'
& cargo build --manifest-path "$repoRoot/tools/java-tc-integration/rust-client/Cargo.toml" *> "$evidenceDir/build.txt"
if ($LASTEXITCODE -ne 0) {throw 'Rust query tool build failed'}
$probe=Join-Path $env:CARGO_TARGET_DIR 'debug/seata-java-tc-it.exe'
$probeHash=(Get-FileHash -LiteralPath $probe).Hash
$helperClass=Join-Path $testRoot 'target/test-classes/org/apache/seata/server/session/FileRetryTimeoutStoreProbe.class'
$helperHash=(Get-FileHash -LiteralPath $helperClass).Hash
$script:activeTc=$null;$script:activeFixture=$null;$results=@();$succeeded=$false

function Quote-JavaArg([string]$value) {return '"'+$value.Replace('\','\\').Replace('"','\"')+'"'}
function Stop-Owned([System.Diagnostics.Process]$process) {
    if ($process) {
        $process.Refresh()
        if (!$process.HasExited) {$process.Kill();if (!$process.WaitForExit(10000)) {throw 'Owned process failed to exit'}}
    }
}
function Run-Fixture([string]$mode,[string]$store,[int]$status,[string]$runtime,[string]$caseDir) {
    $argPath=Join-Path $runtime "$mode.args"
    $arguments=@('-Djava.io.tmpdir=D:/Temp',(Quote-JavaArg "-Duser.home=$runtime"),'-cp',(Quote-JavaArg $classpath),'org.apache.seata.server.session.FileRetryTimeoutStoreProbe',$mode,(Quote-JavaArg $store),[string]$status)
    [System.IO.File]::WriteAllText($argPath,($arguments -join "`n")+"`n",[System.Text.UTF8Encoding]::new($false))
    $script:activeFixture=Start-Process -FilePath $javaExe -WindowStyle Hidden -PassThru -WorkingDirectory $runtime `
        -RedirectStandardOutput "$caseDir/$mode-stdout.log" -RedirectStandardError "$caseDir/$mode-stderr.log" -ArgumentList ('"@{0}"' -f $argPath)
    if (!$script:activeFixture.WaitForExit(30000)) {throw 'Fixture JVM timed out'}
    $script:activeFixture.Refresh()
    if ($script:activeFixture.ExitCode -ne 0) {throw "Fixture JVM failed: $mode/$status"}
    return $script:activeFixture.Id
}

try {
    foreach ($status in @(1,16,17)) {
        $runtime=Join-Path $runtimeRoot "status_$status"
        $caseDir=Join-Path $evidenceDir "status_$status"
        New-Item -ItemType Directory -Path $runtime,$caseDir | Out-Null
        $store=Join-Path $runtime 'data/sessionStore/28091'
        $seedPid=Run-Fixture 'seed' $store $status $runtime $caseDir
        $seedText=Get-Content "$caseDir/seed-stdout.log" -Raw
        $match=[regex]::Match($seedText,"SEEDED status=$status branches=1 xid=(\S+) branchId=(\d+)")
        if (!$match.Success) {throw 'No real FILE seed identity'}
        $xid=$match.Groups[1].Value;$branchId=[long]$match.Groups[2].Value
        $dataFile=Join-Path $store 'root.data'
        $seedHash=(Get-FileHash -LiteralPath $dataFile).Hash
        $seedBytes=(Get-Item -LiteralPath $dataFile).Length
        if ($seedBytes -le 0) {throw 'No persisted data'}
        Copy-Item -LiteralPath $dataFile -Destination "$caseDir/seed-root.data"
        $inspectPid=Run-Fixture 'inspect' $store $status $runtime $caseDir
        if ($seedPid -eq $inspectPid) {throw 'Cannot demonstrate separate persistence JVMs'}
        if ((Get-FileHash -LiteralPath $dataFile).Hash -ne $seedHash) {throw 'Inspection modified seeded bytes'}
        $inspectText=Get-Content "$caseDir/inspect-stdout.log" -Raw
        $expectedRestore=if ($status -eq 17) {'RESTORED sessions=0 branches=0 status=absent'} else {"RESTORED sessions=1 branches=1 status=$status"}
        if (!$inspectText.Contains($expectedRestore)) {throw 'Unexpected actual FILE restore'}
        if ($status -eq 16 -and !$inspectText.Contains('RELOAD_SWITCH=UNHANDLED_16')) {throw 'No actual retry-timeout reload exception'}
        $env:SEATA_TC_WORK_DIR=$runtime.Replace('\','/')
        $config=(Get-Content "$repoRoot/tools/java-tc-integration/config/application.yml" -Raw).Replace('18091','28091')
        $config | Set-Content "$runtime/application.yml"
        Copy-Item "$runtime/application.yml" "$caseDir/application.yml"
        $configUri='file:'+$runtime.Replace('\','/')+'/application.yml'
        $script:activeTc=Start-Process -FilePath $javaExe -WindowStyle Hidden -PassThru -WorkingDirectory $runtime `
            -RedirectStandardOutput "$caseDir/tc-stdout.log" -RedirectStandardError "$caseDir/tc-stderr.log" `
            -ArgumentList @('-Djava.io.tmpdir=D:/Temp',('"-Duser.home={0}"' -f $runtime),('"-Dspring.config.location={0}"' -f $configUri),'-jar',('"{0}"' -f $jar),'-h','127.0.0.1')
        $tcPid=$script:activeTc.Id;$ready=$false;$deadline=(Get-Date).AddSeconds(90)
        do {
            $script:activeTc.Refresh()
            if ($script:activeTc.HasExited) {break}
            $listener=Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue
            if ($listener -and $listener.OwningProcess -eq $tcPid) {$ready=$true;break}
            if ((Get-Date) -ge $deadline) {throw 'TC startup observation timed out'}
            Start-Sleep -Milliseconds 200
        } while ($true)
        $clientExit=$null;$observed=$null;$lockable=$null;$startupExit=$null
        if ($status -eq 16) {
            if ($ready -or !$script:activeTc.HasExited -or $script:activeTc.ExitCode -eq 0) {throw 'Expected pinned CommitRetryTimeout startup failure not reproduced'}
            $startupExit=$script:activeTc.ExitCode
            $allLogs=(Get-Content "$caseDir/tc-stdout.log" -Raw)+(Get-Content "$caseDir/tc-stderr.log" -Raw)
            if (!$allLogs.Contains('ShouldNeverHappenException') -or !$allLogs.Contains('NOT properly handled CommitRetryTimeout')) {throw 'Startup failed for a different cause'}
        } else {
            if (!$ready) {throw 'Control/RollbackRetryTimeout server did not start'}
            $env:SEATA_IT_TIMEOUT_XID=$xid
            $expected=if ($status -eq 1) {1} else {15}
            $env:SEATA_IT_TIMEOUT_EXPECT_STATUS=[string]$expected
            & $probe --query-file-timeout *> "$caseDir/client.txt"
            $clientExit=$LASTEXITCODE
            $text=Get-Content "$caseDir/client.txt" -Raw
            $lockable=($status -eq 17);$observed=$expected
            if ($clientExit -ne 0 -or !$text.Contains("PASS FILE_QUERY xid=$xid status=$expected") -or !$text.Contains("lockable=$($lockable.ToString().ToLower()) targetMutationCount=0")) {throw 'Native old-XID/lock query failed'}
            $script:activeTc.Refresh()
            if ($script:activeTc.HasExited) {throw 'TC exited after native query'}
        }
        Stop-Owned $script:activeTc
        if (Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue) {throw 'Owned TC port not released'}
        $results+=[ordered]@{seeded_status=$status;xid=$xid;branch_id=$branchId;seed_jvm_pid=$seedPid;inspect_jvm_pid=$inspectPid;tc_pid=$tcPid;seed_bytes=$seedBytes;seed_sha256=$seedHash;restored_sessions=if ($status -eq 17) {0} else {1};restored_branches=if ($status -eq 17) {0} else {1};startup_ready=$ready;startup_exit=$startupExit;native_status=$observed;lockable=$lockable;client_exit=$clientExit;diagnostic_matched=$true;recovery_acceptance_pass=($status -eq 1);port_released=$true}
        $script:activeTc=$null
        Write-Output "DIAGNOSTIC matched seededStatus=$status xid=$xid ready=$ready nativeStatus=$observed lockable=$lockable"
    }
    $succeeded=$true
} finally {
    $cleanupErrors=@()
    foreach ($owned in @($script:activeFixture,$script:activeTc)) {try {Stop-Owned $owned} catch {$cleanupErrors+=$_.Exception.Message}}
    $released=!(Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue)
    $finalSuccess=$succeeded -and $released -and $cleanupErrors.Count -eq 0
    [ordered]@{executed_at=(Get-Date).ToString('o');diagnostics_succeeded=$finalSuccess;java_pin=$javaPin;server_jar_sha256=$jarHash;rust_probe_sha256=$probeHash;fixture_class_sha256=$helperHash;fresh_java_tests=5;java_report_sha256=(Get-FileHash -LiteralPath "$evidenceDir/java-process-report.xml").Hash;port_released=$released;cleanup_errors=$cleanupErrors;scenarios=$results;scope='Seeded real FILE records, separate JVMs then unchanged full TC JAR; diagnostic reproduction, not recovery acceptance or naturally scheduled timeout/DB/Undo/operator retry proof'} | ConvertTo-Json -Depth 6 | Set-Content "$evidenceDir/result.json"
    [ordered]@{evidence_directory=$evidenceDir;runtime_directory=$runtimeRoot;diagnostics_succeeded=$finalSuccess} | ConvertTo-Json | Set-Content "$evidenceRoot/latest_run.json"
    if ($cleanupErrors.Count) {throw ('Owned process cleanup failed: '+($cleanupErrors -join '; '))}
    if (!$released) {throw 'Test port not released'}
}
Get-Content "$evidenceDir/result.json"
