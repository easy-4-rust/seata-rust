param(
    [ValidateSet('1.99','1.85')][string]$RustVersion='1.99',
    [ValidateSet('launcher_source_schema_preserves_full_xid_and_rejects_invalid_claim_states','legacy_008_row_remains_unowned_after_009_migration','begin_source_and_end_markers_precede_real_commit_frame','marker_exit_retains_one_way_send_claim','supervised_exit_allows_only_one_owner_transfer','transferred_rollback_lost_reply_stays_unknown','marker_ack_loss_after_persisted_cas_never_emits_end_frame','changed_owner_instance_blocks_original_end_cas','commit_response_loss_keeps_send_claim_and_denies_memory_recovery','snapshot_rejects_same_digest_with_different_full_xid')]
    [string]$TestName='begin_source_and_end_markers_precede_real_commit_frame'
)
$ErrorActionPreference='Stop'
$image='sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902'
$container='seata-tm-launcher-'+[guid]::NewGuid().ToString('N').Substring(0,12)
$database='undo_tm_'+[guid]::NewGuid().ToString('N').Substring(0,12)
$password=[guid]::NewGuid().ToString('N')
$created=$false
try {
    docker image inspect $image --format '{{.Id}}' | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Pinned MySQL image missing'}
    docker run -d --rm --pull=never --name $container --label 'seata.fixture.owner=tm-launcher' -p '127.0.0.1:43318:3306' -e "MYSQL_ROOT_PASSWORD=$password" -e 'MYSQL_ROOT_HOST=%' $image | Out-Null
    if($LASTEXITCODE -ne 0){throw 'MySQL container start failed'}
    $created=$true
    $ready=$false
    for($i=0;$i -lt 45;$i++){
        docker exec $container mysql --protocol=TCP --host=127.0.0.1 --port=3306 --user=root "--password=$password" -N -B -e 'SELECT 1' 2>$null | Out-Null
        if($LASTEXITCODE -eq 0){$ready=$true;break}
        Start-Sleep -Seconds 2
    }
    if(-not $ready){throw 'MySQL not ready'}
    docker exec $container mysql --protocol=TCP --host=127.0.0.1 --port=3306 --user=root "--password=$password" -e "CREATE DATABASE $database CHARACTER SET utf8mb4 COLLATE utf8mb4_bin" | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Test database creation failed'}
    $env:SEATA_MYSQL_PORT='43318'
    $env:SEATA_MYSQL_PASSWORD=$password
    $env:SEATA_MYSQL_DATABASE=$database
    $env:CARGO_PROFILE_TEST_DEBUG='0'
    $env:CARGO_INCREMENTAL='0'
    $env:CARGO_TARGET_DIR=if($RustVersion -eq '1.85'){'E:\workspaces\workspace-github-easy-4-rust\.seata-target-nodebug-185'}else{'E:\workspaces\workspace-github-easy-4-rust\.seata-target-nodebug'}
    $target=if($TestName -in @('launcher_source_schema_preserves_full_xid_and_rejects_invalid_claim_states','legacy_008_row_remains_unowned_after_009_migration')){'live_tm_launcher_schema'}else{'live_tm_launcher_store'}
    cargo "+$RustVersion.0" test -p seata-db-mysql --test $target --offline --locked -- --ignored --exact $TestName --nocapture
    if($LASTEXITCODE -ne 0){throw 'TM Launcher store test failed'}
} finally {
    if($created){docker rm -f $container | Out-Null}
}
