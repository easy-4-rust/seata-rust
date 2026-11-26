param([ValidateSet('1.99.0', '1.85.0')][string]$RustVersion = '1.99.0')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$oracle = Join-Path (Split-Path $repo) 'seata-java-phase09-oracle'
$pin = 'cba73e180fd8896a99df6415b859d275122feaa7'
$image = 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
$run = [guid]::NewGuid().ToString('N').Substring(0, 12)
$db = "undo_identity_$run"
$work = "D:/codex-builds/seata-physical-identity-$run"
$out = Join-Path $repo "docs/phase-2/evidence/tc-lock-physical-identity/run-$RustVersion-$run"
$password = 'seata-mysql-fixture-only'
$ports = @()
$containers = @()
New-Item -ItemType Directory -Path $work,$out | Out-Null
Write-Output $out

try {
    $ddl = git -C $oracle cat-file blob "${pin}:script/client/at/db/mysql.sql"
    if ($LASTEXITCODE -ne 0) { throw 'Fixed Undo schema unavailable.' }
    $ddlPath = Join-Path $work 'client-schema.sql'
    [IO.File]::WriteAllText($ddlPath, ($ddl -join "`n") + "`n")
    foreach ($role in @('primary', 'alternate')) {
        $name = "seata-physical-$run-$role"
        $id = docker run -d --rm --pull=never --name $name --label "seata.fixture.owner=$run" `
            -p '127.0.0.1::3306' -e "MYSQL_ROOT_PASSWORD=$password" -e 'MYSQL_ROOT_HOST=%' `
            -e "MYSQL_DATABASE=$db" $image
        if ($LASTEXITCODE -ne 0) { throw "Cannot initialize owned $role instance." }
        $containers += $id
        $identity = docker inspect $id | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0 -or $identity.Image -ne $image -or $identity.Config.Labels.'seata.fixture.owner' -ne $run) {
            throw "Owned $role identity differs."
        }
        $binding = $identity.NetworkSettings.Ports.'3306/tcp'
        if ($binding.Count -ne 1 -or $binding.HostIp -ne '127.0.0.1') { throw 'Invalid host binding.' }
        $ports += $binding.HostPort
        $ready = $false
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            docker exec -e "MYSQL_PWD=$password" $id mysql --protocol=TCP --host=127.0.0.1 --user=root "--database=$db" '--execute=SELECT 1' 2>$null | Out-Null
            if ($LASTEXITCODE -eq 0) { $ready = $true; break }
            Start-Sleep -Seconds 1
        }
        if (-not $ready) { throw "Owned $role instance did not start." }
    }
    $env:SEATA_MYSQL_PORT = $ports[0]
    $env:SEATA_TEST_SECOND_MYSQL_PORT = $ports[1]
    $env:SEATA_MYSQL_PASSWORD = $password
    $env:SEATA_MYSQL_DATABASE = $db
    $env:SEATA_UNDO_SCHEMA_PATH = $ddlPath
    $env:SEATA_UNDO_STORE_OUT = $work
    $env:TEMP = 'D:/Temp'
    $env:TMP = 'D:/Temp'
    $env:CARGO_HOME = Join-Path (Split-Path $repo) '.seata-cargo-cache'
    $target = if ($RustVersion -eq '1.85.0') { '.seata-target-nodebug-185' } else { '.seata-target-nodebug' }
    $env:CARGO_TARGET_DIR = Join-Path (Split-Path $repo) $target
    $env:CARGO_PROFILE_TEST_DEBUG = '0'
    $env:CARGO_INCREMENTAL = '0'
    Push-Location $repo
    try {
        cargo "+$RustVersion" test --locked --offline -p seata-db-mysql --test live_mysql_physical_identity -- --ignored --exact different_mysql_servers_cannot_share_a_preparation_or_pool 2>&1 |
            Tee-Object -FilePath (Join-Path $out 'test.log')
        $exitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
    $observed = Join-Path $work 'physical-identity.json'
    if (Test-Path -LiteralPath $observed) {
        Copy-Item -LiteralPath $observed -Destination $out
    }
} finally {
    $removed = @()
    foreach ($id in $containers) {
        docker rm -f $id | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Owned MySQL container was not removed. $id" }
        $removed += $id
    }
    @{removed = $removed; mysql_image = $image; ports = $ports; java_schema_pin = $pin} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'runtime.json')
}
exit $exitCode
