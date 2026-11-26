param()

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath "$PSScriptRoot/../../../..").Path
$manifest = Get-Content -LiteralPath "$repo/docs/phase-2/wp05-java-tc-changed-commit-report-attempt.json" -Raw | ConvertFrom-Json
if ($manifest.status -ne 'JAVA_TC_CHANGED_REPORT_LIVE_NOT_RUN') { throw 'Unexpected attempt status' }
if ($manifest.live_verified -ne $false) { throw 'Live verification must remain false' }
foreach ($item in $manifest.sha256.PSObject.Properties) {
    $actual = (Get-FileHash -LiteralPath (Join-Path $repo $item.Name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $item.Value) { throw "Attempt evidence SHA256 mismatch: $($item.Name)" }
}
foreach ($version in @('1.99', '1.85')) {
    $compile = Get-Content -LiteralPath "$PSScriptRoot/java-changed-report-compile-$version.log" -Raw
    $clippy = Get-Content -LiteralPath "$PSScriptRoot/java-changed-report-clippy-$version.log" -Raw
    if ($compile -notmatch 'Executable tests\\live_commit_check_java.rs' -or $clippy -notmatch 'Finished `dev` profile') {
        throw "Missing static verification: Rust $version"
    }
}
Write-Output 'Java TC changed COMMIT/report static preparation verified; live gate NOT RUN.'
