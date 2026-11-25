param(
  [int]$Port = 9222,
  [string]$Profile = "$env:LOCALAPPDATA\com.rustify.spike",
  [string]$Label = 'policy'
)

$ErrorActionPreference = 'Stop'
$exe = 'C:\Rustify\bench\wryhost\target\release\wryhost.exe'
$log = "C:\Rustify\bench\$Label-low.log"
$toggle = "C:\Rustify\bench\$Label.toggle.txt"
Remove-Item $log, $toggle -ErrorAction SilentlyContinue
Set-Content -Path $toggle -Value 'normal' -NoNewline

Get-Process wryhost -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

$p = Start-Process -FilePath $exe -ArgumentList @(
  '--url', 'https://open.spotify.com/',
  '--data-dir', $Profile,
  '--log', $log,
  '--title', "Rustify $Label",
  '--cdp', "$Port",
  '--toggle-file', $toggle,
  '--hide-on-close'
) -PassThru
"pid=$($p.Id) toggle=$toggle"
Start-Sleep -Seconds 25

# open a playlist context so playback has a real queue
& node C:\Rustify\bench\measure\cdp-open-playlist.mjs
Start-Sleep -Seconds 8

$state = & node C:\Rustify\bench\measure\cdp-state.mjs $Port | ConvertFrom-Json
if ($state.playpause_label -eq 'Pause') {
  "pausing_before_trim"
  & node C:\Rustify\bench\measure\cdp-click.mjs $Port
  Start-Sleep -Seconds 5
}

"--- paused, normal, before trim ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $p.Id -Seconds 10 -Label "${Label}_paused_normal"

Set-Content -Path $toggle -Value 'low' -NoNewline
"low_set_at=$((Get-Date).ToUniversalTime().ToString('o'))"
Start-Sleep -Seconds 45
"--- paused, low, settled ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $p.Id -Seconds 20 -Label "${Label}_paused_low"
& node C:\Rustify\bench\measure\cdp-metrics.mjs $Port

# play with Low retained: the tuned policy candidate
& node C:\Rustify\bench\measure\cdp-play-playlist.mjs
Start-Sleep -Seconds 10
"--- playback check ---"
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\Rustify\bench\measure\smtc.ps1
"ready_for_soak root=$($p.Id)"
