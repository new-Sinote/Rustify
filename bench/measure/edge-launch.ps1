param(
  [int]$Port = 9223,
  [string]$Profile = 'C:\Rustify\bench\edge-profile'
)

$ErrorActionPreference = 'Stop'
$edge = 'C:\Program Files (x86)\Microsoft\EdgeCore\158.0.4370.33\msedge.exe'
if (-not (Test-Path $edge)) { throw "Edge executable not found at $edge" }
New-Item -ItemType Directory -Force -Path $Profile | Out-Null

Get-CimInstance Win32_Process | Where-Object {
  $_.Name -eq 'msedge.exe' -and $_.CommandLine -match [regex]::Escape($Profile)
} | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2

$args = @(
  "--user-data-dir=$Profile",
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-sync',
  "--remote-debugging-port=$Port",
  '--app=https://open.spotify.com/',
  '--window-size=1296,839'
)
$launcher = Start-Process -FilePath $edge -ArgumentList $args -PassThru
Start-Sleep -Seconds 12

$procs = Get-CimInstance Win32_Process | Where-Object {
  $_.Name -eq 'msedge.exe' -and $_.CommandLine -match [regex]::Escape($Profile)
}
$browser = $procs | Where-Object { $_.CommandLine -notmatch '--type=' } | Select-Object -First 1
if (-not $browser) { throw 'No Edge browser process found for the bench profile.' }
$browser.Id | Set-Content 'C:\Rustify\bench\edge-browser.pid'
"browser_pid=$($browser.Id) launcher_pid=$($launcher.Id) profile=$Profile port=$Port"
Get-Process -Id $browser.Id | Select-Object Id, MainWindowTitle, MainWindowHandle | Format-List
"--- cdp targets ---"
try {
  $targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 5
  $targets | Where-Object { $_.type -eq 'page' } | ForEach-Object { "title=$($_.title) url=$($_.url)" }
} catch { "cdp_error: $($_.Exception.Message)" }
