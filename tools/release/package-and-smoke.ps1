[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Push-Location $root
try {
  if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Windows x64 is required"
  }
  cargo build --release
  if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
  cargo test --workspace --all-targets --all-features --release
  if ($LASTEXITCODE -ne 0) { throw "Release tests failed" }
  python tools/validation/contracts.py package windows-x64
  if ($LASTEXITCODE -ne 0) { throw "package contract failed" }
} finally {
  Pop-Location
}
