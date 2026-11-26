$ErrorActionPreference='Stop'
$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'
$env:SEATA_MYSQL_DATABASE='undo_current_constraints'
$env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$env:SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql'
$env:SEATA_UNDO_STORE_OUT='D:/codex-builds/undo-current-constraints/red'

$e='docs/phase-2/evidence/mysql-undo-current-constraints'
foreach($v in '1.99','1.85'){
 $env:SEATA_UNDO_STORE_OUT="D:/codex-builds/undo-current-constraints/$v"
 New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 cargo build --locked --offline -p seata-db-mysql --example mysql_rm_recovery_child *> "$e/$v-child-build.log"
 if($LASTEXITCODE){Get-Content "$e/$v-child-build.log" -Tail 20;exit $LASTEXITCODE}
 $env:SEATA_RM_CHILD_EXE="$env:CARGO_TARGET_DIR/debug/examples/mysql_rm_recovery_child.exe"
 $suites=@('live_undo_rollback','live_rm_recovery')
 foreach($suite in $suites){
 $runTag=[DateTime]::UtcNow.ToString('yyyyMMddHHmmssfff')
 $env:SEATA_UNDO_STORE_OUT="D:/codex-builds/undo-current-constraints/$v-$suite-$runTag"
 if(Test-Path -LiteralPath $env:SEATA_UNDO_STORE_OUT){throw 'fresh output directory required'}
 New-Item -ItemType Directory $env:SEATA_UNDO_STORE_OUT | Out-Null
 $env:SEATA_UNDO_STORE_OUT | Set-Content "$e/$v-$suite-output-path.txt"

 if($suite -eq 'live_rm_recovery'){
 wsl.exe -d Ubuntu-20.04 --exec docker -H unix:///run/seata-validation-20261120/docker.sock exec -e MYSQL_PWD=seata-mysql-fixture-only seata-wsl-undo-constraints-20261123 mysql --host=127.0.0.1 --port=43317 --user=root undo_current_constraints -e "DROP USER IF EXISTS 'rm_terminal_denied'@'%'"
 if($LASTEXITCODE){exit $LASTEXITCODE}
 }
 cargo test --locked --offline -p seata-db-mysql --test $suite -- --ignored --test-threads=1 *> "$e/$v-$suite.log"
 if($LASTEXITCODE){Get-Content "$e/$v-$suite.log" -Tail 30;exit $LASTEXITCODE}
 Get-Content "$e/$v-$suite.log" | Select-String 'test result:'
 }
}
