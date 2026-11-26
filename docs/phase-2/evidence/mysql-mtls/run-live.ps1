$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'
$env:SEATA_MYSQL_DATABASE='undo_mtls'
$env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$env:SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql'
$env:SEATA_UNDO_STORE_OUT='D:/codex-builds/undo-current-constraints/red'

$env:SEATA_TLS_CA='D:/codex-builds/mysql-mtls-20261123/certs/ca.pem'
$env:SEATA_TLS_OTHER_CA='D:/codex-builds/mysql-mtls-20261123/certs/other-ca.pem'

$env:SEATA_MTLS_CERTS='D:/codex-builds/mysql-mtls-20261123/certs'
$e='docs/phase-2/evidence/mysql-mtls'
foreach($v in '1.99','1.85'){
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 cargo test --locked --offline -p seata-db-mysql --test options_contract *> "$e/$v-options.log"
 if($LASTEXITCODE){Get-Content "$e/$v-options.log" -Tail 15;exit $LASTEXITCODE}
 cargo test --locked --offline -p seata-db-mysql --test live_tls -- --ignored --nocapture --test-threads=1 *> "$e/$v-live.log"
 if($LASTEXITCODE){Get-Content "$e/$v-live.log" -Tail 20;exit $LASTEXITCODE}
 Get-Content "$e/$v-live.log" | Select-String 'cipher=|observed|test result:'
}
