$ErrorActionPreference='Stop'
$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$env:SEATA_MYSQL_PORT='43318'; $env:SEATA_MYSQL_DATABASE='undo_wsl_validation'; $env:SEATA_MYSQL_PASSWORD='seata-mysql-fixture-only'
$env:SEATA_UNDO_SCHEMA_PATH='D:/codex-builds/seata-wsl-mysql-20261120/init/00-undo.sql'
$e=(Resolve-Path 'docs/phase-2/evidence/fastjson-undo-native/run_20261120_owned_wsl_mysql').Path
cargo fmt --all -- --check *> (Join-Path $e 'fmt.log')
if($LASTEXITCODE){exit $LASTEXITCODE}
foreach($v in '1.99','1.85'){
  if ($v -eq '1.85'){
    $rustBin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
    $env:Path="$rustBin;$env:Path"
    $env:RUSTC="$rustBin/rustc.exe"; $env:RUSTDOC="$rustBin/rustdoc.exe"
    $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc'
    $env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
  }
  foreach($target in 'live_undo_rollback','live_callback_ownership_loss'){
    $env:SEATA_UNDO_STORE_OUT="D:/codex-builds/seata-wsl-mysql-20261120/$v-$target"
    New-Item -ItemType Directory -Force $env:SEATA_UNDO_STORE_OUT | Out-Null
    $testFilter=@()
    if($target -eq 'live_callback_ownership_loss'){
      $testFilter=@('compensation_acquisition_rejection_preserves_normal_until_tc_retries','--exact')
    }
    $log=Join-Path $e "$v-$target.log"
    cargo test --locked --offline -p seata-db-mysql --test $target -- @testFilter --ignored --nocapture --test-threads=1 *> $log
    if($LASTEXITCODE){Get-Content $log -Tail 40;exit $LASTEXITCODE}
    Copy-Item -LiteralPath $env:SEATA_UNDO_STORE_OUT -Destination $e -Recurse
    Get-Content $log | Select-String 'test result:' | ForEach-Object { "$v $target $_" }
  }
  cargo clippy --locked --offline --workspace --all-features --all-targets -- -D warnings *> (Join-Path $e "$v-clippy.log")
  if($LASTEXITCODE){Get-Content (Join-Path $e "$v-clippy.log") -Tail 30;exit $LASTEXITCODE}
  cargo test --locked --offline --workspace --all-features --all-targets *> (Join-Path $e "$v-workspace.log")
  if($LASTEXITCODE){Get-Content (Join-Path $e "$v-workspace.log") -Tail 30;exit $LASTEXITCODE}
  cargo test --locked --offline --workspace --all-features --doc *> (Join-Path $e "$v-doc.log")
  if($LASTEXITCODE){exit $LASTEXITCODE}
}
