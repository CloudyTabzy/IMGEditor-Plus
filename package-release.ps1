param(
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinaryPath = Join-Path $ProjectRoot "target\release\imgeditor.exe"
$DistPath = Join-Path $ProjectRoot $OutputDir

Write-Host "Building release binary..."
& cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "Release build failed"
}

if (-not (Test-Path $BinaryPath)) {
    throw "Expected binary not found at $BinaryPath"
}

Write-Host "Creating distribution directory..."
New-Item -ItemType Directory -Force -Path $DistPath | Out-Null

Write-Host "Copying binary and docs..."
Copy-Item -Path $BinaryPath -Destination $DistPath -Force
if (Test-Path (Join-Path $ProjectRoot "docs")) {
    Copy-Item -Path (Join-Path $ProjectRoot "docs") -Destination $DistPath -Recurse -Force
}

Write-Host "Release packaged to $DistPath"
