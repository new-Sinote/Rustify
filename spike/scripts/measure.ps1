param(
    [Parameter(Mandatory = $true)][int]$RootId,
    [int]$Seconds = 10
)

$ErrorActionPreference = 'Stop'

function Get-Descendants {
    param([int]$ParentId, $Processes)
    $children = $Processes | Where-Object { $_.ParentProcessId -eq $ParentId }
    foreach ($child in $children) {
        $child
        Get-Descendants -ParentId $child.ProcessId -Processes $Processes
    }
}

$snapshot = Get-CimInstance Win32_Process
$tree = @(Get-Descendants -ParentId $RootId -Processes $snapshot)
$root = $snapshot | Where-Object { $_.ProcessId -eq $RootId }
$all = @($root) + $tree

# Include WebView2 processes whose user-data-dir belongs to this app, even if reparented.
$webview = $snapshot | Where-Object {
    $_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -and
    ($_.CommandLine -match 'com\.rustify\.spike' -or $_.CommandLine -match 'RustifySpike')
}
$all = @($all) + $webview
$ids = $all.ProcessId | Sort-Object -Unique

$first = Get-Process -Id $ids -ErrorAction SilentlyContinue |
    Select-Object Id, ProcessName, WorkingSet64, PrivateMemorySize64, CPU
Start-Sleep -Seconds $Seconds
$second = Get-Process -Id $ids -ErrorAction SilentlyContinue |
    Select-Object Id, ProcessName, WorkingSet64, PrivateMemorySize64, CPU

$cpuDelta = 0.0
foreach ($proc in $second) {
    $before = $first | Where-Object { $_.Id -eq $proc.Id } | Select-Object -First 1
    if ($before -and $null -ne $proc.CPU -and $null -ne $before.CPU) {
        $cpuDelta += ($proc.CPU - $before.CPU)
    }
}

$cores = [Environment]::ProcessorCount
$workingSetMb = [math]::Round((($second | Measure-Object WorkingSet64 -Sum).Sum) / 1MB, 1)
$privateMb = [math]::Round((($second | Measure-Object PrivateMemorySize64 -Sum).Sum) / 1MB, 1)
$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains $_.IDProcess }
$privateWsMb = [math]::Round((($perf | Measure-Object WorkingSetPrivate -Sum).Sum) / 1MB, 1)
$cpuOneCore = [math]::Round(($cpuDelta / $Seconds) * 100, 2)
$cpuAllCores = [math]::Round($cpuOneCore / $cores, 2)

"tree_processes=$($second.Count)"
"working_set_mb=$workingSetMb"
"private_working_set_mb=$privateWsMb"
"private_commit_mb=$privateMb"
"cpu_pct_one_core=$cpuOneCore"
"cpu_pct_all_cores=$cpuAllCores"
"logical_cores=$cores"
"processes:"
$second | Sort-Object WorkingSet64 -Descending | ForEach-Object {
    "  {0} pid={1} ws_mb={2} private_mb={3}" -f $_.ProcessName, $_.Id,
        [math]::Round($_.WorkingSet64 / 1MB, 1),
        [math]::Round($_.PrivateMemorySize64 / 1MB, 1)
}
