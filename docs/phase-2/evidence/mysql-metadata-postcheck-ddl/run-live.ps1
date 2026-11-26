$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'
$env:SEATA_MYSQL_DATABASE='undo_postcheck_ddl'
$env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$e='docs/phase-2/evidence/mysql-metadata-postcheck-ddl'
foreach($v in '1.99','1.85'){
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 cargo test --locked --offline -p seata-db-mysql --test live_metadata -- checked_metadata_blocks_later_trigger_and_foreign_key_ddl_until_rollback --ignored --exact *> "$e/$v-live.log"
 if($LASTEXITCODE){Get-Content "$e/$v-live.log" -Tail 20;exit $LASTEXITCODE}
 Get-Content "$e/$v-live.log" | Select-String 'test result:'
}
