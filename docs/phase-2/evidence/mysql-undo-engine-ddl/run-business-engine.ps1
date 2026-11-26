$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'
$env:SEATA_MYSQL_DATABASE='undo_engine_ddl'
$env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$env:SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql'
$env:SEATA_UNDO_STORE_OUT='D:/codex-builds/undo-engine-ddl'
New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null

$e='docs/phase-2/evidence/mysql-undo-engine-ddl'
foreach($v in '1.99','1.85'){
 $env:SEATA_UNDO_STORE_OUT="D:/codex-builds/undo-engine-ddl/$v"
 New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 foreach($suite in 'live_metadata'){
 cargo test --locked --offline -p seata-db-mysql --test $suite -- engine_only_ddl_is_rejected_before_nontransactional_business_execution --ignored --exact --nocapture *> "$e/$v-business-engine.log"
 if($LASTEXITCODE){Get-Content "$e/$v-business-engine.log" -Tail 30;exit $LASTEXITCODE}
 Get-Content "$e/$v-business-engine.log" | Select-String 'test result:'
 }
}
