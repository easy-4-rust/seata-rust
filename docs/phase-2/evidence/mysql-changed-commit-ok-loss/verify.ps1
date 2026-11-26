param()

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath "$PSScriptRoot/../../../..").Path
$manifest = Get-Content -LiteralPath "$repo/docs/phase-2/wp05-mysql-changed-commit-ok-loss.json" -Raw | ConvertFrom-Json
if ($manifest.status -ne 'EXPERIMENTAL_CHANGED_COMMIT_SERVER_OK_LOSS_VERIFIED_ONLY') {
    throw 'Unexpected evidence status'
}
$sourceDrift = @()
foreach ($item in $manifest.sha256.PSObject.Properties) {
    $path = Join-Path $repo $item.Name
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $item.Value) {
        if ($item.Name -notlike 'docs/phase-2/evidence/mysql-changed-commit-ok-loss/*') {
            $sourceDrift += $item.Name
        } else {
            throw "Archived evidence SHA256 mismatch: $($item.Name)"
        }
    }
}
foreach ($version in @('1.99', '1.85')) {
    $live = Get-Content -LiteralPath "$PSScriptRoot/changed-commit-loss-$version-live.log" -Raw
    $regression = Get-Content -LiteralPath "$PSScriptRoot/changed-commit-loss-$version-regression.log" -Raw
    $marker = Get-Content -LiteralPath "$PSScriptRoot/changed-commit-loss-$version-ok" -Raw
    if ($live -notmatch '1 passed; 0 failed' -or $regression -notmatch '21 passed; 0 failed' -or $marker -notmatch 'original MySQL COMMIT returned OK; reply held') {
        throw "Missing successful evidence: Rust $version"
    }
    foreach ($mode in @('default', 'lab')) {
        $clippy = Get-Content -LiteralPath "$PSScriptRoot/changed-loss-$version-$mode-clippy.log" -Raw
        if ($clippy -notmatch 'Finished `dev` profile') {
            throw "Missing Clippy completion: Rust $version $mode"
        }
    }
}
Write-Output "Archived changed COMMIT OK-loss evidence verified; historical source files advanced: $($sourceDrift.Count). Full WP05 gate remains open."
