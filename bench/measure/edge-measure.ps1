param(
  [int]$Port = 9223,
  [string]$Label = 'edge_web_player',
  [string]$OutDir = 'C:\Rustify\bench\edge-shots'
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$pidPath = 'C:\Rustify\bench\edge-browser.pid'
if (-not (Test-Path $pidPath)) { throw 'Run edge-launch.ps1 first.' }
$rootId = [int](Get-Content $pidPath)
if (-not (Get-Process -Id $rootId -ErrorAction SilentlyContinue)) { throw "Edge browser $rootId is not running." }

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class EdgeWin {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[EdgeWin]::SetProcessDPIAware() | Out-Null
$h = (Get-Process -Id $rootId).MainWindowHandle
[EdgeWin]::SetWindowPos($h, [IntPtr](-1), 120, 90, 1296, 839, 0x0040) | Out-Null
[EdgeWin]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Seconds 3
$r = New-Object EdgeWin+RECT
[EdgeWin]::GetWindowRect($h, [ref]$r) | Out-Null
"rect=$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"

function Shot($path) {
  $bmp = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

Shot (Join-Path $OutDir "$Label-paused.png")
"--- paused 1280x800 ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $rootId -Seconds 15 -Label "${Label}_paused"
& node C:\Rustify\bench\measure\cdp-metrics.mjs $Port

& node C:\Rustify\bench\measure\cdp-click.mjs $Port
Start-Sleep -Seconds 10
Shot (Join-Path $OutDir "$Label-playing.png")
"--- playing 1280x800 ---"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $rootId -Seconds 20 -Label "${Label}_playing"
& node C:\Rustify\bench\measure\cdp-metrics.mjs $Port
