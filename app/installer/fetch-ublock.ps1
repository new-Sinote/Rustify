param(
  [string]$Version = "2026.1124.1811",
  [string]$Sha256 = "b0472c07028ced638cc0ae7c57403dd95ad6cdfc7391a37f9fa7aaf6239dfc55",
  [string]$OutDir = "",
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$installerDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $OutDir) {
  $OutDir = Join-Path $installerDir "ublock-payload"
}

$manifestPath = Join-Path $OutDir "manifest.json"
if (-not $Force -and (Test-Path $manifestPath)) {
  $existing = Get-Content $manifestPath -Raw | ConvertFrom-Json
  if ($existing.version -eq $Version) {
    "ublock_payload=present version=$Version"
    exit 0
  }
}

$zipName = "uBOLite_$Version.chromium.zip"
$url = "https://github.com/uBlockOrigin/uBOL-home/releases/download/$Version/$zipName"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("rustify-ubol-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
  $zip = Join-Path $tmp $zipName
  & curl.exe -L --fail --silent --show-error -o $zip $url
  if ($LASTEXITCODE -ne 0) { throw "download failed: $url" }

  $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -ne $Sha256.ToLowerInvariant()) {
    throw "sha256 mismatch for $zipName`: got $hash, expected $Sha256"
  }

  $extract = Join-Path $tmp "extract"
  Expand-Archive -Path $zip -DestinationPath $extract -Force

  $manifest = Get-ChildItem $extract -Recurse -Filter manifest.json -File |
    Where-Object { Test-Path (Join-Path $_.DirectoryName "rulesets") } |
    Select-Object -First 1
  if (-not $manifest) { throw "manifest.json not found in $zipName" }
  $source = $manifest.DirectoryName

  if (Test-Path $OutDir) { Remove-Item $OutDir -Recurse -Force }
  New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
  Copy-Item -Path (Join-Path $source "*") -Destination $OutDir -Recurse -Force

  $installed = Get-Content (Join-Path $OutDir "manifest.json") -Raw | ConvertFrom-Json
  if ($installed.version -ne $Version) {
    throw "unexpected extension version: $($installed.version)"
  }

  $sourceNote = @"
uBlock Origin Lite $Version
Upstream release: https://github.com/uBlockOrigin/uBOL-home/releases/tag/$Version
Build source: https://github.com/uBlockOrigin/uBOL-home/tree/$Version
uBlock submodule source: https://github.com/gorhill/uBlock/tree/d717684ad6a2b635a192564d27c081c9365c9577
Licence: GPL-3.0 (see LICENSE.txt)
Downloaded asset: $zipName
SHA-256: $hash
"@
  Set-Content -Path (Join-Path $OutDir "SOURCE.txt") -Value $sourceNote -Encoding UTF8

  $files = (Get-ChildItem $OutDir -Recurse -File | Measure-Object).Count
  "ublock_payload=ready version=$Version files=$files sha256=$hash"
} finally {
  if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}
