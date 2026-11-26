param()

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath "$PSScriptRoot/../../../..").Path
$manifest = Get-Content -LiteralPath "$repo/docs/phase-2/wp05-mysql-changed-commit-report.json" -Raw | ConvertFrom-Json
if ($manifest.status -ne 'EXPERIMENTAL_CHANGED_COMMIT_REPORT_VERIFIED_ONLY') {
    throw 'Unexpected evidence status'
}
foreach ($item in $manifest.sha256.PSObject.Properties) {
    $actual = (Get-FileHash -LiteralPath (Join-Path $repo $item.Name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $item.Value) {
        throw "SHA256 mismatch: $($item.Name)"
    }
}
foreach ($version in @('1.99', '1.85')) {
    foreach ($case in @('success', 'loss', 'local-unknown')) {
        $log = Get-Content -LiteralPath "$PSScriptRoot/changed-report-$version-$case.log" -Raw
        if ($log -notmatch '1 passed; 0 failed') { throw "Missing pass: Rust $version $case" }
    }
    $regression = Get-Content -LiteralPath "$PSScriptRoot/changed-report-$version-regression.log" -Raw
    if ($regression -notmatch '23 passed; 0 failed') { throw "Missing regression: Rust $version" }
    foreach ($mode in @('default', 'lab')) {
        $clippy = Get-Content -LiteralPath "$PSScriptRoot/changed-report-$version-$mode-clippy.log" -Raw
        if ($clippy -notmatch 'Finished `dev` profile') { throw "Missing Clippy: Rust $version $mode" }
    }
    $workspace = Get-Content -LiteralPath "$PSScriptRoot/changed-report-$version-workspace-test.log" -Raw
    $results = [regex]::Matches($workspace, 'test result: ok\. (\d+) passed; 0 failed')
    $passed = 0
    foreach ($result in $results) { $passed += [int]$result.Groups[1].Value }
    if ($passed -ne 636) { throw "Workspace regression count mismatch: Rust $version" }
}
$cleanup = Get-Content -LiteralPath "$PSScriptRoot/changed-report-cleanup.log" -Raw
if ($cleanup -notmatch 'Owned engine stopped') { throw 'Missing owned runtime cleanup' }
Write-Output 'Experimental first-phase report evidence verified; formal WP05 gate remains open.'
