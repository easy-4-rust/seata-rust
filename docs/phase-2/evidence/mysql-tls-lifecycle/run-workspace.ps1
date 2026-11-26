$env:TEMP=$env:TMP='D:/codex-builds'
$env:CARGO_HOME='E:/workspaces/workspace-github-easy-4-rust/.seata-cargo-cache'
$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver'
$env:CARGO_INCREMENTAL='0'
$e='docs/phase-2/evidence/mysql-tls-lifecycle'
cargo fmt --all -- --check *> "$e/fmt.log"
if($LASTEXITCODE){exit $LASTEXITCODE}
foreach($v in '1.99','1.85'){
 if($v -eq '1.85'){
 $bin='C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin'
 $env:Path="$bin;$env:Path";$env:RUSTC="$bin/rustc.exe";$env:RUSTDOC="$bin/rustdoc.exe"
 $env:RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc';$env:CARGO_TARGET_DIR='D:/cargo-target/seata-rust-load-driver-msrv'
 }
 cargo clippy --locked --offline --workspace --all-features --all-targets -- -D warnings *> "$e/$v-clippy.log"
 if($LASTEXITCODE){Get-Content "$e/$v-clippy.log" -Tail 15;exit $LASTEXITCODE}
 cargo test --locked --offline --workspace --all-features --all-targets *> "$e/$v-workspace.log"
 if($LASTEXITCODE){Get-Content "$e/$v-workspace.log" -Tail 15;exit $LASTEXITCODE}
 "$v passed"
}

