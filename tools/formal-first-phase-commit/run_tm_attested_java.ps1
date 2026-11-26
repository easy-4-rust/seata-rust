param(
    [Parameter(Mandatory)][string]$ZigExe,
    [ValidateSet('1.99','1.85')][string]$RustVersion='1.99',
    [ValidateSet('tm_attested_at_commit','tm_attested_at_rollback_loss')][string]$Scenario='tm_attested_at_commit'
)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$zig=(Resolve-Path -LiteralPath $ZigExe).Path
if($zig -match '["%!\r\n]'){throw 'Unsupported Zig path'}
$build=Join-Path (Split-Path $repo -Parent) '.seata-node-linux'
$token=[guid]::NewGuid().ToString('N').Substring(0,12)
$stage=Join-Path $build "attested-$token"

function Get-AttestedArtifact([string]$Package,[string]$Target,[string[]]$Options) {
    $messages=cargo +stable test -p $Package --test $Target @Options --no-run --offline --locked --message-format=json
    if($LASTEXITCODE -ne 0){throw "Failed to build $Target"}
    $artifacts=@(foreach($line in $messages){
        $row=$line | ConvertFrom-Json
        if($row.reason -eq 'compiler-artifact' -and $row.target.name -eq $Target -and $row.executable){$row.executable}
    })
    if($artifacts.Count -ne 1){throw "Expected one $Target executable"}
    $artifacts[0]
}

Push-Location $repo
try {
    if(-not (Test-Path -LiteralPath $build)){New-Item -ItemType Directory -Path $build | Out-Null}
    New-Item -ItemType Directory -Path $stage | Out-Null
    $cc=Join-Path $build 'zig-linux.cmd'
    $ar=Join-Path $build 'zig-ar.cmd'
    Set-Content -LiteralPath $cc -Encoding ascii -Value @('@echo off',('"{0}" cc -target x86_64-linux-gnu.2.31 %*' -f $zig))
    Set-Content -LiteralPath $ar -Encoding ascii -Value @('@echo off',('"{0}" ar %*' -f $zig))
    $env:CC_x86_64_unknown_linux_gnu=$cc
    $env:AR_x86_64_unknown_linux_gnu=$ar
    $env:CFLAGS_x86_64_unknown_linux_gnu='-fPIC'
    $env:CRATE_CC_NO_DEFAULTS='1'
    $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$cc
    $env:ZIG_GLOBAL_CACHE_DIR=Join-Path $build 'zig-global'
    $env:ZIG_LOCAL_CACHE_DIR=Join-Path $build 'zig-local'
    $env:CARGO_TARGET_DIR=Join-Path $build 'target'
    $env:CARGO_PROFILE_TEST_DEBUG='0'
    $env:CARGO_INCREMENTAL='0'
    $env:SEATA_TM_NODE_PROVISION_DIR=$stage
    $provision=Get-AttestedArtifact 'seata-tm' 'stop_attestation_tls' @()
    & $provision --exact --ignored node::provision
    if($LASTEXITCODE -ne 0){throw 'Test credential provisioning failed'}
    $env:SEATA_ATTESTED_TEST_LINUX_ARTIFACT=Get-AttestedArtifact 'seata-db-mysql' 'live_tm_launcher_java' @('--target','x86_64-unknown-linux-gnu','--features','experimental_changed_commit')
    $env:SEATA_ATTESTED_TEST_MATERIAL=$stage
    & (Join-Path $PSScriptRoot 'run_desktop.ps1') -RustVersion $RustVersion -Scenario $Scenario
    if($LASTEXITCODE -ne 0){throw "Attested Java fixture failed: $Scenario"}
} finally {
    if(Test-Path -LiteralPath $stage){
        $actual=(Resolve-Path -LiteralPath $stage).Path
        $expected=Join-Path (Resolve-Path -LiteralPath $build).Path "attested-$token"
        if($actual -ne $expected){throw 'Unexpected attestation staging path'}
        Remove-Item -LiteralPath $actual -Recurse -Force
    }
    Pop-Location
}
