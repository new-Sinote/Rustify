param(
  [Parameter(Mandatory = $true)][int]$RootId,
  [int]$Minutes = 35,
  [int]$Port = 9222,
  [string]$OutDir = 'C:\Rustify\bench\soak'
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$snapDir = Join-Path $OutDir 'snapshots'
New-Item -ItemType Directory -Force -Path $snapDir | Out-Null

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class SoakPower {
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
}
"@
[SoakPower]::SetThreadExecutionState([uint32]2147483651) | Out-Null  # ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED

Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]
function Await($WinRtTask, $ResultType) {
  $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
  $netTask = $asTask.Invoke($null, @($WinRtTask))
  $netTask.Wait(-1) | Out-Null
  $netTask.Result
}
$managerType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$propsType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties, Windows.Media.Control, ContentType = WindowsRuntime]
$manager = Await ($managerType::RequestAsync()) $managerType

function Read-Smtc {
  try {
    $session = $manager.GetCurrentSession()
    if (-not $session) { return [pscustomobject]@{ app = ''; title = ''; artist = ''; status = 'NoSession'; position_s = 0; end_s = 0 } }
    $props = Await ($session.TryGetMediaPropertiesAsync()) $propsType
    $info = $session.GetPlaybackInfo()
    $timeline = $session.GetTimelineProperties()
    [pscustomobject]@{
      app = $session.SourceAppUserModelId
      title = $props.Title
      artist = $props.Artist
      status = [string]$info.PlaybackStatus
      position_s = [math]::Round($timeline.Position.TotalSeconds, 1)
      end_s = [math]::Round($timeline.EndTime.TotalSeconds, 1)
    }
  } catch {
    [pscustomobject]@{ app = ''; title = ''; artist = ''; status = 'Error'; position_s = 0; end_s = 0 }
  }
}

$smtcCsv = Join-Path $OutDir 'smtc.csv'
"at,t_min,app,title,artist,status,position_s,end_s" | Set-Content -Path $smtcCsv -Encoding UTF8
$sampleCsv = Join-Path $OutDir 'samples.csv'
"label,t_min,sampled_at,processes,ws_mb,private_ws_mb,private_commit_mb,pss_mb,shared_ws_mb,gpu_mb,cpu_pct_one_core,heap_used_mb,heap_total_mb,embedder_heap_mb,backing_storage_mb,nodes,documents,listeners,visibility,page_title" | Set-Content -Path $sampleCsv -Encoding UTF8

$start = Get-Date
$deadline = $start.AddMinutes($Minutes)
$nextSample = 0.0
$smtcCount = 0
$adSeen = 0
"soak_start root=$RootId minutes=$Minutes at=$($start.ToUniversalTime().ToString('o'))" | TEE-Object -Append -FilePath (Join-Path $OutDir 'soak.log')

while ((Get-Date) -lt $deadline) {
  $t = [math]::Round(((Get-Date) - $start).TotalMinutes, 2)
  $s = Read-Smtc
  $smtcCount++
  if ($s.title -match 'advert') { $adSeen++ }
  Add-Content -Path $smtcCsv -Encoding UTF8 -Value ("{0},{1},{2},{3},{4},{5},{6},{7}" -f `
    (Get-Date).ToUniversalTime().ToString('o'), $t, $s.app, ($s.title -replace ',', ' '), ($s.artist -replace ',', ' '), $s.status, $s.position_s, $s.end_s)

  if ($t -ge $nextSample) {
    $label = ('soak_{0:00}m' -f [math]::Floor($t))
    $treeJson = & C:\Rustify\bench\measure\tree-metrics.ps1 -RootId $RootId -Seconds 10 -Label $label -Json
    $tree = $treeJson | ConvertFrom-Json
    $cdpRaw = & node C:\Rustify\bench\measure\cdp-metrics.mjs $Port 2>&1 | Out-String
    $cdp = $null
    try { $cdp = $cdpRaw | ConvertFrom-Json } catch { $cdp = $null }
    $treeJson | Set-Content -Path (Join-Path $snapDir "$label-tree.json") -Encoding UTF8
    $cdpRaw | Set-Content -Path (Join-Path $snapDir "$label-cdp.json") -Encoding UTF8
    $vis = if ($cdp -and $cdp.page) { $cdp.page.visibility } else { '' }
    $heap = if ($cdp -and $cdp.heap) { $cdp.heap.used_mb } else { 0 }
    $heapTotal = if ($cdp -and $cdp.heap) { $cdp.heap.total_mb } else { 0 }
    $embed = if ($cdp -and $cdp.heap) { $cdp.heap.embedder_used_mb } else { 0 }
    $backing = if ($cdp -and $cdp.heap) { $cdp.heap.backing_storage_mb } else { 0 }
    $nodes = if ($cdp -and $cdp.dom) { $cdp.dom.nodes } else { 0 }
    $docs = if ($cdp -and $cdp.dom) { $cdp.dom.documents } else { 0 }
    $listeners = if ($cdp -and $cdp.dom) { $cdp.dom.js_event_listeners } else { 0 }
    $ptitle = if ($cdp -and $cdp.page) { ($cdp.page.title -replace ',', ' ') } else { '' }
    Add-Content -Path $sampleCsv -Encoding UTF8 -Value ("{0},{1},{2},{3},{4},{5},{6},{7},{8},{9},{10},{11},{12},{13},{14},{15},{16},{17},{18},{19}" -f `
      $label, $t, $tree.summary.sampled_at, $tree.summary.processes, $tree.summary.ws_mb, $tree.summary.private_ws_mb, $tree.summary.private_commit_mb, $tree.summary.pss_mb, $tree.summary.shared_ws_mb, $tree.summary.gpu_mb, $tree.summary.cpu_pct_one_core, $heap, $heapTotal, $embed, $backing, $nodes, $docs, $listeners, $vis, $ptitle)
    "sample $label pss=$($tree.summary.pss_mb) priv=$($tree.summary.private_ws_mb) ws=$($tree.summary.ws_mb) cpu=$($tree.summary.cpu_pct_one_core) heap=$heap nodes=$nodes status=$($s.status) title=$($s.title)" | TEE-Object -Append -FilePath (Join-Path $OutDir 'soak.log')
    $nextSample += 5.0
  }

  $sleep = 12 - (((Get-Date) - $start).TotalSeconds % 12)
  Start-Sleep -Milliseconds ([int]([math]::Max(500, $sleep * 1000)))
}

$endSample = Read-Smtc
"soak_end t=$([math]::Round(((Get-Date) - $start).TotalMinutes,2)) ad_rows=$adSeen smtc_rows=$smtcCount last=$($endSample.title) $($endSample.status) pos=$($endSample.position_s)" | TEE-Object -Append -FilePath (Join-Path $OutDir 'soak.log')
