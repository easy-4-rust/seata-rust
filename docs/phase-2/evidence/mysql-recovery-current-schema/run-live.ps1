$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'
$env:SEATA_MYSQL_DATABASE='undo_recovery_current_schema'
$env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$env:SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql'
$env:SEATA_UNDO_STORE_OUT='D:/codex-builds/recovery-current-schema'
New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null

$e='docs/phase-2/evidence/mysql-recovery-current-schema'
foreach($v in '1.99','1.85'){
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 cargo build --locked --offline -p seata-db-mysql --example mysql_rm_recovery_child *> "$e/$v-child-build.log"
 if($LASTEXITCODE){exit $LASTEXITCODE}
 $env:SEATA_RM_CHILD_EXE="$env:CARGO_TARGET_DIR/debug/examples/mysql_rm_recovery_child.exe"
 foreach($suite in 'live_commit_preconditions','live_first_phase_preparation','live_rm_recovery','live_commit_direction'){
 if($suite -eq 'live_first_phase_preparation'){
 wsl -d Ubuntu-20.04 -- docker -H unix:///run/seata-validation-20261120/docker.sock exec -e MYSQL_PWD=seata-mysql-fixture-only seata-wsl-recovery-current-schema-20261123 mysql --host=127.0.0.1 --port=43317 --user=root -N -B -e 'DROP DATABASE IF EXISTS undo_recovery_current_schema_other'
 if($LASTEXITCODE){exit $LASTEXITCODE}
 }
 if($suite -eq 'live_rm_recovery'){
 wsl -d Ubuntu-20.04 -- docker -H unix:///run/seata-validation-20261120/docker.sock exec -e MYSQL_PWD=seata-mysql-fixture-only seata-wsl-recovery-current-schema-20261123 mysql --host=127.0.0.1 --port=43317 --user=root -N -B -e "DROP USER IF EXISTS 'rm_terminal_denied'@'%'"
 if($LASTEXITCODE){exit $LASTEXITCODE}
 }
 $tag=[DateTime]::UtcNow.ToString('yyyyMMddHHmmssfff')
 $env:SEATA_UNDO_STORE_OUT="D:/codex-builds/recovery-current-schema/$v-$suite-$tag"
 New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null
 $env:SEATA_UNDO_STORE_OUT | Set-Content "$e/$v-$suite-output-path.txt"
 $filter=@()
 if($suite -eq 'live_commit_direction'){$filter=@('real_commit_callback_atomically_cleans_root_children_and_terminal_without_business_changes','--exact')}
 cargo test --locked --offline -p seata-db-mysql --test $suite -- @filter --ignored --nocapture --test-threads=1 *> "$e/$v-$suite.log"
 if($LASTEXITCODE){Get-Content "$e/$v-$suite.log" -Tail 30;exit $LASTEXITCODE}
 Get-Content "$e/$v-$suite.log" | Select-String 'test result:'
 }
}



