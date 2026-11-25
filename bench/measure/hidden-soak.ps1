param(
  [Parameter(Mandatory = $true)][int]$RootId,
  [int]$Minutes = 10,
  [int]$Port = 9222,
  [string]$OutDir = 'C:\Rustify\bench\soak-hidden'
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$csv = Join-Path $OutDir 'samples.csv'
"label,t_min,sampled_at,processes,ws_mb,private_ws_mb,pss_mb,cpu_pct_one_core,window_visible,smtc_status,smtc_title" |
  Set-Content -Path $csv -Encoding UTF8

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class HiddenSoak {
  public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);

  // GetProcess.MainWindowHandle can point at a Tao message window after hiding.
  // Find the real top-level window by title instead.
  public static bool? IsAppVisible(uint pid) {
    IntPtr found = IntPtr.Zero;
    bool visible = false;
    EnumWindows((h, l) => {
      uint owner;
      GetWindowThreadProcessId(h, out owner);
      if (owner != pid) return true;
      var text = new StringBuilder(256);
      GetWindowTextW(h, text, 256);
      if (text.ToString() == "Rustify") {
        found = h;
        visible = IsWindowVisible(h);
        return false;
      }
      return true;
    }, IntPtr.Zero);
    if (found == IntPtr.Zero) return null;
    return visible;
  }
}
"@
[HiddenSoak]::SetThreadExecutionState([uint32]2147483651) | Out-Null  # keep system and display awake

$start = Get-Date
$deadline = $start.AddMinutes($Minutes)
$index = 0
$rows = @()

while ((Get-Date) -lt $deadline) {
  $label = 'hidden_{0:00}' -f $index
  $tree = & C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 10 -Label $label -Json | ConvertFrom-Json

  $smtc = $null
  try {
    $raw = & powershell -NoProfile -ExecutionPolicy Bypass -File C:\Rustify\bench\measure\smtc.ps1 2>$null
    $smtc = $raw | ConvertFrom-Json
  } catch { }

  $visible = [HiddenSoak]::IsAppVisible([uint32]$RootId)
  if ($null -eq $visible) { $visible = 'unknown' }

  $status = if ($smtc -and $smtc.status) { $smtc.status } else { '' }
  $title = if ($smtc -and $smtc.title) { ($smtc.title -replace ',', ' ') } else { '' }
  $t = [math]::Round(((Get-Date) - $start).TotalMinutes, 2)

  Add-Content -Path $csv -Encoding UTF8 -Value ("{0},{1},{2},{3},{4},{5},{6},{7},{8},{9},{10}" -f `
    $label, $t, $tree.summary.sampled_at, $tree.summary.processes, $tree.summary.ws_mb,
    $tree.summary.private_ws_mb, $tree.summary.pss_mb, $tree.summary.cpu_pct_one_core,
    $visible, $status, $title)

  $rows += [pscustomobject]@{
    label = $label
    t_min = $t
    pss = [double]$tree.summary.pss_mb
    cpu = [double]$tree.summary.cpu_pct_one_core
    visible = $visible
    status = $status
  }
  "$label t=$t pss=$($tree.summary.pss_mb) cpu=$($tree.summary.cpu_pct_one_core) visible=$visible status=$status" |
    Tee-Object -Append -FilePath (Join-Path $OutDir 'hidden.log')

  $index++
  $elapsed = ((Get-Date) - $start).TotalSeconds
  $next = $start.AddSeconds($index * 60)
  $sleep = [math]::Max(0, ($next - (Get-Date)).TotalMilliseconds)
  if ($sleep -gt 0) { Start-Sleep -Milliseconds ([int]$sleep) }
  if ((Get-Date) -ge $deadline) { break }
}

$maxPss = ($rows | Measure-Object -Property pss -Maximum).Maximum
$meanPss = [math]::Round(($rows | Measure-Object -Property pss -Average).Average, 1)
$maxCpu = ($rows | Measure-Object -Property cpu -Maximum).Maximum
$meanCpu = [math]::Round(($rows | Measure-Object -Property cpu -Average).Average, 2)
$allHidden = -not ($rows | Where-Object { $_.visible })
"summary max_pss=$maxPss mean_pss=$meanPss max_cpu=$maxCpu mean_cpu=$meanCpu all_hidden=$allHidden" |
  Tee-Object -Append -FilePath (Join-Path $OutDir 'hidden.log')

if ($maxPss -gt 300) { Write-Error "paused PSS exceeded 300 MB: $maxPss" }
if ($maxCpu -gt 1) { Write-Error "paused CPU exceeded 1% of one core: $maxCpu" }
