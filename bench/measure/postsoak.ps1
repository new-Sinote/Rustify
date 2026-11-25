param(
  [Parameter(Mandatory = $true)][int]$RootId,
  [int]$Port = 9222,
  [string]$Label = 'wry_postsoak'
)

$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class PostWin {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
$h = (Get-Process -Id $RootId).MainWindowHandle

function PageState {
  & node C:\Rustify\bench\measure\cdp-state.mjs $Port | ConvertFrom-Json
}

"--- playing, visible, before hide ---"
$before = PageState
"bar=$($before.bar_text -replace "`n", ' | ')"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 15 -Label "${Label}_visible_playing"

[PostWin]::ShowWindow($h, 0) | Out-Null  # SW_HIDE
"hidden_at=$((Get-Date).ToUniversalTime().ToString('o'))"
Start-Sleep -Seconds 65
$after = PageState
"bar_after=$($after.bar_text -replace "`n", ' | ')"
"visibility=$($after.visibility)"
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 20 -Label "${Label}_hidden_playing"
& node C:\Rustify\bench\measure\cdp-metrics.mjs $Port

[PostWin]::ShowWindow($h, 9) | Out-Null  # SW_RESTORE
[PostWin]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Seconds 5
"--- paused for Low settle ---"
& node C:\Rustify\bench\measure\cdp-click.mjs $Port
Start-Sleep -Seconds 60
& C:\Rustify\bench\measure\cdp-state.mjs $Port
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 30 -Label "${Label}_paused_low_settled"
"--- resume ---"
& node C:\Rustify\bench\measure\cdp-click.mjs $Port
Start-Sleep -Seconds 12
& C:\Rustify\bench\measure\cdp-state.mjs $Port
& C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 20 -Label "${Label}_resumed_playing"
