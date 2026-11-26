param(
    [ValidateSet('1.99', '1.85')]
    [string]$RustVersion = '1.99',
    [ValidateSet('no_change_commit', 'no_change_ack_loss', 'no_change_pool_recovery', 'changed_pool_recovery', 'tm_attested_at_commit', 'tm_attested_at_rollback_loss', 'tm_recovered_at_commit', 'tm_recovered_at_rollback_loss', 'tm_recovered_commit', 'tm_recovered_rollback_loss', 'tm_durable_commit_loss', 'tm_durable_rollback_loss', 'rollback_orphan_missing_root', 'rollback_orphan_guarded_root', 'commit_callback_reply_loss', 'rollback_callback_reply_loss', 'rollback_missing_undo', 'rollback_missing_undo_reply_loss', 'commit', 'commit_recovered', 'commit_cross_process', 'armed_child_business_commit', 'armed_child_business_commit_ack', 'armed_child_commit_in_flight', 'armed_child_fence_ack_loss', 'armed_child_query_failure', 'armed_child_arm_ack_loss', 'armed_child_arm_blocked', 'armed_child_wait_rollback', 'armed_child_wait_commit', 'commit_cross_process_race', 'commit_distinct_process_race', 'commit_report_loss', 'commit_report_claim_loss', 'commit_report_claim_child_exit', 'failed', 'report_loss', 'undo_error', 'undo_report_loss', 'undo_duplicate', 'enroll_error', 'enroll_duplicate', 'enroll_duplicate_report_loss',
        'commit_db_block_terminal', 'commit_db_block_commit', 'commit_db_in_flight', 'commit_db_drop_ok',
        'rollback_db_block_terminal', 'rollback_db_block_commit', 'rollback_db_in_flight', 'rollback_db_drop_ok', 'rollback_dirty',
        'commit_mixed', 'rollback_mixed', 'tls_wrong_ca', 'commit_permission', 'undo_checksum', 'undo_checksum_rollback', 'undo_checksum_legacy', 'graceful_rm_restart')]
    [string]$Scenario = 'commit_distinct_process_race'
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$image = 'sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
$container = 'seata-java-report-' + [guid]::NewGuid().ToString('N').Substring(0, 12)
$created = $false
$exitCode = 1
$tlsFolder = $null

try {
    docker image inspect $image --format '{{.Id}}' | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned MySQL image is missing. Pull mysql@$image before running the fixture."
    }
    $dockerMount = @()
    $mysqlOptions = @()
    if ($Scenario -eq 'tls_wrong_ca') {
        $tlsFolder = Join-Path 'D:\codex-builds' ($container + '-tls')
        python -B (Join-Path $PSScriptRoot 'mysql_tls_material.py') $tlsFolder 'D:\Java\jdk-17.0.12\bin\keytool.exe'
        if ($LASTEXITCODE -ne 0) {
            throw 'Unable to create disposable MySQL TLS materials.'
        }
        $env:SEATA_JAVA_MYSQL_TLS = $tlsFolder
        $dockerMount = @('--mount', "type=bind,source=$tlsFolder,target=/mysql-tls,readonly")
        $mysqlOptions = @('--require-secure-transport=ON', '--ssl-ca=/mysql-tls/ca.pem',
            '--ssl-cert=/mysql-tls/server.pem', '--ssl-key=/mysql-tls/server-key.pem')
    }
    docker run -d --rm --pull=never --name $container `
        --label 'seata.fixture.owner=java-tc-report' `
        -p '127.0.0.1:43318:3306' `
        -e 'MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only' `
        -e 'MYSQL_ROOT_HOST=%' @dockerMount $image @mysqlOptions | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'Unable to start the owned MySQL container.'
    }
    $created = $true

    # 临时初始化服务可能仅监听 Unix socket，须等宿主 TCP 上的 MySQL 握手。
    $ready = $false
    for ($attempt = 0; $attempt -lt 45; $attempt++) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new()
            $client.Connect('127.0.0.1', 43318)
            $client.ReceiveTimeout = 1500
            $header = New-Object byte[] 4
            $count = $client.GetStream().Read($header, 0, 4)
            $client.Dispose()
            if ($count -eq 4 -and $header[3] -eq 0) {
                $ready = $true
                break
            }
        } catch {
            if ($null -ne $client) {
                $client.Dispose()
            }
        }
        Start-Sleep -Seconds 2
    }
    if (-not $ready) {
        throw 'Owned MySQL host handshake timed out.'
    }

    $env:SEATA_MYSQL_BACKEND = 'desktop'
    $env:SEATA_MYSQL_CONTAINER = $container
    $env:SEATA_CARGO_TARGET_DIR = 'E:\workspaces\workspace-github-easy-4-rust\.seata-target-nodebug'
    $env:SEATA_CARGO_TARGET_DIR_MSRV = 'E:\workspaces\workspace-github-easy-4-rust\.seata-target-nodebug-185'
    $env:CARGO_PROFILE_TEST_DEBUG = '0'
    $env:CARGO_INCREMENTAL = '0'
    $env:PYTHONDONTWRITEBYTECODE = '1'
    Push-Location $repoRoot
    try {
        python -B tools/formal-first-phase-commit/java_tc_report.py $RustVersion $Scenario
        $exitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
} finally {
    if ($created) {
        docker rm -f $container | Out-Null
    }
    if ($null -ne $tlsFolder -and (Test-Path -LiteralPath $tlsFolder -PathType Container)) {
        foreach ($file in @('ca.pem', 'wrong-ca.pem', 'server.pem', 'server-key.pem', 'trust.p12')) {
            $material = Join-Path $tlsFolder $file
            if (Test-Path -LiteralPath $material -PathType Leaf) {
                Remove-Item -LiteralPath $material
            }
        }
        [System.IO.Directory]::Delete($tlsFolder)
    }
}
exit $exitCode
