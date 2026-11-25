param(
  [switch]$SkipBuild,
  [string]$Makensis = ""
)

$ErrorActionPreference = "Stop"
$appDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$appDir = (Resolve-Path $appDir).Path
$distDir = Join-Path $appDir "dist"

if (-not $SkipBuild) {
  Push-Location $appDir
  try {
    # Cargo writes progress to stderr; Windows PowerShell 5.1 turns that into a
    # terminating error under ErrorActionPreference=Stop even on success.
    $ErrorActionPreference = "Continue"
    cargo build --release
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = "Stop"
    if ($buildExit -ne 0) { throw "cargo build failed with exit code $buildExit" }
  } finally {
    $ErrorActionPreference = "Stop"
    Pop-Location
  }
}

$exe = Join-Path $appDir "target\release\rustify.exe"
if (-not (Test-Path $exe)) { throw "missing $exe" }

if (-not $Makensis) {
  $candidates = @(@(
    $env:MAKENSIS,
    "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe",
    "C:\Program Files (x86)\NSIS\makensis.exe",
    "C:\Program Files\NSIS\makensis.exe"
  ) | Where-Object { $_ -and (Test-Path $_) })
  if ($candidates.Count -gt 0) {
    $Makensis = $candidates[0]
  } else {
    $cmd = Get-Command makensis -ErrorAction SilentlyContinue
    if ($cmd) { $Makensis = $cmd.Source }
  }
}
if (-not $Makensis -or -not (Test-Path $Makensis)) {
  throw "makensis.exe not found; pass -Makensis <path>"
}

New-Item -ItemType Directory -Force -Path $distDir | Out-Null
$setup = Join-Path $distDir "Rustify-0.2.0-x64-setup.exe"
if (Test-Path $setup) { Remove-Item $setup -Force }

Push-Location (Join-Path $appDir "installer")
try {
  & $Makensis "rustify.nsi"
  if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
} finally {
  Pop-Location
}

$hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
$name = Split-Path $setup -Leaf
Set-Content -Path (Join-Path $distDir "SHA256SUMS.txt") -Encoding ASCII -NoNewline -Value "$hash  $name`n"

$size = (Get-Item $setup).Length
"built={0} bytes={1} sha256={2}" -f $setup, $size, $hash
