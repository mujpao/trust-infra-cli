# Smoke test for the trustinfra CLI. Run from the crate root in PowerShell:
#   powershell -ExecutionPolicy Bypass -File .\smoke.ps1
# Adjust $Bin if your binary has a different name.

$Bin  = "cargo run -q --"
$Work = Join-Path $env:TEMP "trustinfra-smoke"
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Path "$Work\bundle\artifacts" | Out-Null

# .NET writers: UTF-8 without BOM, no trailing newline, works on Windows PowerShell 5.1 and 7+
function WriteText($path, $text)  { [System.IO.File]::WriteAllText($path, $text) }
function AppendText($path, $text) { [System.IO.File]::AppendAllText($path, $text) }

# --- sample unsigned bundle (sha256 is a placeholder; `sign` fills it in) ---
WriteText "$Work\bundle\artifacts\results.json" '{"accuracy_pct": 91}'
WriteText "$Work\bundle\artifacts\scorer.py"    '# scorer'
WriteText "$Work\bundle\artifacts\dataset.json" '[1,2,3]'

$zero = "0" * 64
$manifest = @"
{
  "schema_version": "0.1",
  "run_id": "smoke-001",
  "timestamp": "2026-11-14T00:00:00Z",
  "model": "test-model",
  "scorer_commit": "abc123",
  "judge_model": "test-judge",
  "artifacts": {
    "results": { "path": "artifacts/results.json", "sha256": "$zero" },
    "scorer":  { "path": "artifacts/scorer.py",    "sha256": "$zero" },
    "dataset": { "path": "artifacts/dataset.json", "sha256": "$zero" }
  },
  "infra_declared": { "image": "sha256:aaa", "gpu": "none" },
  "infra_actual":   { "image": "sha256:aaa", "gpu": "none" }
}
"@
WriteText "$Work\bundle\manifest.json" $manifest

# Sanity check: stop immediately if setup didn't produce the files.
foreach ($f in @("manifest.json", "artifacts\results.json", "artifacts\scorer.py", "artifacts\dataset.json")) {
    if (-not (Test-Path "$Work\bundle\$f")) { Write-Host "SETUP FAILED: missing $f" -ForegroundColor Red; exit 99 }
}

$script:Failures = 0
function Run($label, $expected, [scriptblock]$cmd) {
    Write-Host "`n=== $label (expect exit $expected) ===" -ForegroundColor Cyan
    & $cmd
    $code = $LASTEXITCODE
    if ($code -eq $expected) { Write-Host "OK: exit $code" -ForegroundColor Green }
    else { Write-Host "MISMATCH: got exit $code" -ForegroundColor Red; $script:Failures++ }
}

$key = "$Work\k"; $bundle = "$Work\bundle"

Run "keygen"                         0 { Invoke-Expression "$Bin keygen $key" }
Run "keygen refuses to overwrite"    1 { Invoke-Expression "$Bin keygen $key" }
Run "sign"                           0 { Invoke-Expression "$Bin sign $bundle --key $key.key" }
Run "verify (clean)"                 0 { Invoke-Expression "$Bin verify $bundle --trusted-key $key.pub" }
Run "verify --json (clean)"          0 { Invoke-Expression "$Bin verify $bundle --trusted-key $key.pub --json" }

# wrong key must not verify
Run "keygen (attacker)"              0 { Invoke-Expression "$Bin keygen $Work\evil" }
Run "verify with wrong key"          1 { Invoke-Expression "$Bin verify $bundle --trusted-key $Work\evil.pub" }

# missing key file
Run "verify with missing key file"   2 { Invoke-Expression "$Bin verify $bundle --trusted-key $Work\nope.pub" }

# unlisted file
WriteText "$bundle\artifacts\extra.txt" "extra"
Run "verify with extra file"         1 { Invoke-Expression "$Bin verify $bundle --trusted-key $key.pub" }
Remove-Item "$bundle\artifacts\extra.txt"

# back to clean after removing the extra file
Run "verify (clean again)"           0 { Invoke-Expression "$Bin verify $bundle --trusted-key $key.pub" }

# tamper an artifact
AppendText "$bundle\artifacts\results.json" "x"
Run "verify after editing results"   1 { Invoke-Expression "$Bin verify $bundle --trusted-key $key.pub" }

Write-Host ""
if ($script:Failures -eq 0) { Write-Host "ALL CHECKS OK" -ForegroundColor Green }
else { Write-Host "$($script:Failures) MISMATCH(ES)" -ForegroundColor Red }
Write-Host "Work dir: $Work"
