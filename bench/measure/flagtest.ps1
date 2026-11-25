param(
  [Parameter(Mandatory = $true)][string]$Flags,
  [string]$Label = 'flagtest',
  [int]$Port = 9224,
  [int]$SettleSeconds = 20,
  [string]$OutDir = 'C:\Rustify\bench\flagtests'
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$profile = "$env:LOCALAPPDATA\com.rustify.spike"
$log = Join-Path $OutDir "$Label.log"
$toggle = Join-Path $OutDir "$Label.toggle.txt"
Remove-Item $log, $toggle -ErrorAction SilentlyContinue
Set-Content -Path $toggle -Value 'normal' -NoNewline

$exe = 'C:\Rustify\bench\wryhost\target\release\wryhost.exe'
$p = Start-Process -FilePath $exe -ArgumentList @(
  '--url', 'https://open.spotify.com/',
  '--data-dir', $profile,
  '--log', $log,
  '--title', "Rustify $Label",
  '--browser-args', $Flags,
  '--cdp', "$Port",
  '--toggle-file', $toggle
) -PassThru
"pid=$($p.Id) flags=$Flags"
Start-Sleep -Seconds $SettleSeconds
"--- paused, normal ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $p.Id -Seconds 10 -Label "${Label}_paused_normal"
Set-Content -Path $toggle -Value 'low' -NoNewline
Start-Sleep -Seconds 15
"--- paused, low ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $p.Id -Seconds 10 -Label "${Label}_paused_low"
& node C:\Rustify\bench\measure\cdp-click.mjs $Port
Start-Sleep -Seconds 8
"--- playing, low ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $p.Id -Seconds 20 -Label "${Label}_playing_low"
& node C:\Rustify\bench\measure\cdp-metrics.mjs $Port
"done pid=$($p.Id)"
