# Build an unsigned Lux MSI with WiX 5. No Authenticode certificate is read.
$ErrorActionPreference = "Stop"

$version = $env:LUX_VERSION
if (-not $version) { throw "LUX_VERSION is required (x.y.z)" }
$version = $version.TrimStart("v")
$exe = $env:LUX_BIN
if (-not $exe) { throw "LUX_BIN is required" }
if (-not (Test-Path $exe)) { throw "binary missing: $exe" }
$out = $env:LUX_MSI
if (-not $out) { $out = "dist/lux-windows-x86_64.msi" }

dotnet tool install --global wix --version 5.0.2
$wix = Join-Path $env:USERPROFILE ".dotnet\tools\wix.exe"
if (-not (Test-Path $wix)) { throw "wix.exe was not installed" }

$outDir = Split-Path -Parent $out
if ($outDir -and -not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

& $wix build "$PSScriptRoot/../packaging/windows/lux.wxs" `
    -d "Version=$version" `
    -d "LuxExe=$exe" `
    -o $out
if ($LASTEXITCODE -ne 0) { throw "wix build failed ($LASTEXITCODE)" }
if (-not (Test-Path $out)) { throw "msi was not written: $out" }
Write-Host "packed $out"
