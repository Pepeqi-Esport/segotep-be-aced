# Build Windows x86 + x64 release artifacts into dist\:
#   segotep-be-aced-{version}_portable_windows_64.exe
#   segotep-be-aced-{version}_portable_windows_86.exe
#   segotep-be-aced-{version}_installer_windows_64.exe   (Inno Setup)
#   segotep-be-aced-{version}_installer_windows_86.exe   (Inno Setup)
#
# Requires: Rust MSVC targets, .NET 8 SDK (sensor-bridge), Inno Setup 7 (or 6)
# Usage: cargo release   (or: .\scripts\build-installer.ps1)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Target64 = "x86_64-pc-windows-msvc"
$Target86 = "i686-pc-windows-msvc"
# Prefer the MSVC toolchain even if the default rustup toolchain is GNU.
$Toolchain = "stable-x86_64-pc-windows-msvc"
$CargoPlus = $null  # e.g. "+stable-x86_64-pc-windows-msvc"
$RustupTargetArgs = @()

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

# Native rustup/cargo stderr must not become terminating errors under
# $ErrorActionPreference = "Stop" (PowerShell treats NativeCommandError as errors).
function Invoke-Native {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [string[]] $ArgumentList = @()
    )
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $lines = & $FilePath @ArgumentList 2>&1 | ForEach-Object { "$_" }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Lines    = @($lines)
        }
    } finally {
        $ErrorActionPreference = $prev
    }
}

function Ensure-MsvcToolchain {
    # Avoid `$list -notmatch` — on arrays that filter returns non-matches and is often truthy.
    $list = (Invoke-Native rustup @("toolchain", "list")).Lines
    $has = $list | Where-Object { $_ -like "$Toolchain*" }
    if (-not $has) {
        Write-Host "==> rustup toolchain install $Toolchain" -ForegroundColor Cyan
        # Skip rustup self-update during install (avoids Access Denied on locked rustup.exe).
        $prevAuto = $env:RUSTUP_AUTO_SELF_UPDATE
        $env:RUSTUP_AUTO_SELF_UPDATE = "disable"
        try {
            $r = Invoke-Native rustup @("toolchain", "install", $Toolchain)
        } finally {
            if ($null -eq $prevAuto) { Remove-Item Env:RUSTUP_AUTO_SELF_UPDATE -ErrorAction SilentlyContinue }
            else { $env:RUSTUP_AUTO_SELF_UPDATE = $prevAuto }
        }
        if ($r.ExitCode -ne 0) {
            $r.Lines | ForEach-Object { Write-Host $_ }
            exit $r.ExitCode
        }
    }
    $script:CargoPlus = "+$Toolchain"
    $script:RustupTargetArgs = @("--toolchain", $Toolchain)
    Write-Host "==> using toolchain $Toolchain" -ForegroundColor Cyan
}

function Ensure-RustTarget([string]$Target) {
    $r = Invoke-Native rustup (@("target", "list", "--installed") + $RustupTargetArgs)
    if ($r.ExitCode -ne 0) {
        $r.Lines | ForEach-Object { Write-Host $_ }
        exit $r.ExitCode
    }
    if ($r.Lines -notcontains $Target) {
        Write-Host "==> rustup target add $Target" -ForegroundColor Cyan
        $prevAuto = $env:RUSTUP_AUTO_SELF_UPDATE
        $env:RUSTUP_AUTO_SELF_UPDATE = "disable"
        try {
            $add = Invoke-Native rustup (@("target", "add", $Target) + $RustupTargetArgs)
        } finally {
            if ($null -eq $prevAuto) { Remove-Item Env:RUSTUP_AUTO_SELF_UPDATE -ErrorAction SilentlyContinue }
            else { $env:RUSTUP_AUTO_SELF_UPDATE = $prevAuto }
        }
        if ($add.ExitCode -ne 0) {
            $add.Lines | ForEach-Object { Write-Host $_ }
            exit $add.ExitCode
        }
    }
}

function Build-Portable([string]$Target, [string]$DestName) {
    Write-Host "==> cargo $CargoPlus build --release --target $Target" -ForegroundColor Cyan
    & cargo $CargoPlus build --release --target $Target
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $src = Join-Path $Root "target\$Target\release\segotep-be-aced.exe"
    if (-not (Test-Path $src)) {
        throw "Release binary missing: $src"
    }

    $dest = Join-Path $Root "dist\$DestName"
    Copy-Item -Force $src $dest
    Write-Host "==> portable: dist\$DestName" -ForegroundColor Green
}

function Build-Installer([string]$Iscc, [string]$Version, [string]$Arch, [string]$ExpectedName) {
    Write-Host "==> ISCC MyArch=$Arch → $ExpectedName" -ForegroundColor Cyan
    & $Iscc "/DMyAppVersion=$Version" "/DMyArch=$Arch" (Join-Path $Root "installer\be-aced.iss")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $out = Join-Path $Root "dist\$ExpectedName"
    if (-not (Test-Path $out)) {
        throw "Installer missing after ISCC: $out"
    }
    Write-Host "==> installer: dist\$ExpectedName" -ForegroundColor Green
}

Ensure-MsvcToolchain
Ensure-RustTarget $Target64
Ensure-RustTarget $Target86

$cargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
} else {
    $version = "1.0.0"
}

$Portable64 = "segotep-be-aced-${version}_portable_windows_64.exe"
$Portable86 = "segotep-be-aced-${version}_portable_windows_86.exe"
$Installer64 = "segotep-be-aced-${version}_installer_windows_64.exe"
$Installer86 = "segotep-be-aced-${version}_installer_windows_86.exe"

$dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

# Remove previous release artifacts with old naming so dist\ stays clean.
Get-ChildItem $dist -Filter "BeAced-Setup-*.exe" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem $dist -Filter "segotep-be-aced_*.exe" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem $dist -Filter "segotep-be-aced-*.exe" -ErrorAction SilentlyContinue | Remove-Item -Force

Build-Portable $Target64 $Portable64
Build-Portable $Target86 $Portable86

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

Build-Installer $iscc $version "64" $Installer64
Build-Installer $iscc $version "86" $Installer86

Write-Host "==> release artifacts in dist\" -ForegroundColor Green
Write-Host "    $Portable64"
Write-Host "    $Portable86"
Write-Host "    $Installer64"
Write-Host "    $Installer86"
