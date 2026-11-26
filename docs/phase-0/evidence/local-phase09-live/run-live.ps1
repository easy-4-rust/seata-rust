# Reproduce the test against an already built, pinned Java oracle; no production service.
param(
    [string]$JavaSource = 'E:\workspaces\workspace-github-easy-4-rust\seata-java-phase09-oracle',
    [string]$RuntimeDir = 'D:\codex-builds\seata-phase09-live-20261111'
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path "$PSScriptRoot/../../../..").Path
$expectedJava = 'cba73e180fd8896a99df6415b859d275122feaa7'
$actualJava = & git -C $JavaSource rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actualJava -ne $expectedJava) { throw 'Java source SHA mismatch' }
if ($RuntimeDir -notmatch '^[DE]:\\') { throw 'Runtime directory must be on D: or E:' }
$jar = Join-Path $JavaSource 'server/target/seata-server.jar'
if (!(Test-Path -LiteralPath $jar)) { throw 'Build the pinned release-seata-jar profile first' }
if (Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue) {
    throw 'Test port 28091 is already in use'
}
New-Item -ItemType Directory -Path $RuntimeDir -Force | Out-Null
Copy-Item -LiteralPath "$PSScriptRoot/application.yml" -Destination "$RuntimeDir/application.yml"
$env:SEATA_TC_WORK_DIR = $RuntimeDir.Replace('\', '/')
$env:SEATA_TC_HOST = '127.0.0.1'
$env:SEATA_TC_PORT = '28091'
$env:CARGO_TARGET_DIR = 'D:\cargo-target\seata-phase09'
$env:TEMP = 'D:\Temp'
$env:TMP = 'D:\Temp'
$config = 'file:' + $RuntimeDir.Replace('\', '/') + '/application.yml'
$tc = $null
$clientExit = $null
try {
    $tc = Start-Process -FilePath 'D:\Java\jdk-17.0.12\bin\java.exe' -ArgumentList @(
        '-Djava.io.tmpdir=D:/Temp', "-Duser.home=$RuntimeDir",
        "-Dspring.config.location=$config", '-jar', $jar
    ) -WorkingDirectory $RuntimeDir -WindowStyle Hidden -PassThru `
      -RedirectStandardOutput "$PSScriptRoot/tc-stdout.log" `
      -RedirectStandardError "$PSScriptRoot/tc-stderr.log"
    $deadline = (Get-Date).AddSeconds(90)
    do {
        $tc.Refresh()
        if ($tc.HasExited) { throw 'Java TC exited before readiness; inspect tc logs' }
        $listener = Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue
        if ($listener -and $listener.OwningProcess -eq $tc.Id) { break }
        if ((Get-Date) -ge $deadline) { throw 'Java TC readiness timed out' }
        Start-Sleep -Milliseconds 500
    } while ($true)
    & cargo run --manifest-path "$repoRoot/tools/java-tc-integration/rust-client/Cargo.toml" -- --lifecycle *> "$PSScriptRoot/lifecycle.txt"
    $clientExit = $LASTEXITCODE
    if ($clientExit -ne 0) { throw "Lifecycle probe failed with exit $clientExit; inspect lifecycle.txt" }
} finally {
    if ($tc) {
        $tc.Refresh()
        if (!$tc.HasExited) { Stop-Process -Id $tc.Id; $tc.WaitForExit(10000) | Out-Null }
    }
    $remaining = @(Get-NetTCPConnection -LocalPort 28091 -State Listen -ErrorAction SilentlyContinue)
    [ordered]@{
        executed_at = (Get-Date).ToString('o')
        java_source = $actualJava
        java_executable = 'D:\Java\jdk-17.0.12\bin\java.exe'
        server_jar_sha256 = (Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash
        runtime_directory = $RuntimeDir
        config_sha256 = (Get-FileHash "$PSScriptRoot/application.yml" -Algorithm SHA256).Hash
        tc_pid = $(if ($tc) { $tc.Id } else { $null })
        lifecycle_exit = $clientExit
        stop_method = 'Windows Stop-Process (forced); graceful shutdown not tested'
        port_released = ($remaining.Count -eq 0)
        scope = 'Auth-disabled FILE TC; empty transactions only; no branch/AT/restart proof'
    } | ConvertTo-Json | Set-Content "$PSScriptRoot/live-result.json"
}
Get-Content "$PSScriptRoot/lifecycle.txt"
Get-Content "$PSScriptRoot/live-result.json"
