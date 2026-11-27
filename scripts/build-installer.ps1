# Build release binary + Inno Setup installer → dist\BeAced-Setup-*.exe
# Requires: Rust toolchain, .NET 8 SDK (for sensor-bridge), Inno Setup 7 (or 6)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Find-ISCC {
    $candidates = @(
        "${env:ProgramFiles}\Inno Setup 7\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
        "${env:LocalAppData}\Programs\Inno Setup 7\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:LocalAppData}\Programs\Inno Setup 6\ISCC.exe"
    )
    foreach ($path in $candidates) {
        if (Test-Path $path) { return $path }
    }
    $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    return $null
}

Write-Host "==> cargo build --release" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $Root "target\release\segotep-be-aced.exe"
if (-not (Test-Path $exe)) {
    throw "Release binary missing: $exe"
}

$ico = Join-Path $Root "installer\app.ico"
if (-not (Test-Path $ico)) {
    Write-Host "==> warning: installer\app.ico missing (run cargo build once on Windows to generate it)" -ForegroundColor Yellow
}

$iscc = Find-ISCC
if (-not $iscc) {
    throw @"
Inno Setup not found (expected 7, also accepts 6).
Install from https://jrsoftware.org/isinfo.php then re-run this script.
"@
}

$cargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
} else {
    $version = "1.0.0"
}

Write-Host "==> ISCC: $iscc (version $version)" -ForegroundColor Cyan
& $iscc "/DMyAppVersion=$version" (Join-Path $Root "installer\be-aced.iss")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$out = Get-ChildItem (Join-Path $Root "dist\BeAced-Setup-*.exe") | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($out) {
    Write-Host "==> Installer ready: $($out.FullName)" -ForegroundColor Green
} else {
    Write-Host "==> ISCC finished (check dist\)" -ForegroundColor Green
}
