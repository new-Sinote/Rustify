param(
    [Parameter(Mandatory = $true)][int]$RootId,
    [int]$Seconds = 15,
    [string]$Label = "",
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$pageSize = 4096

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class PssReader {
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint access, bool inherit, int pid);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr handle);

    [DllImport("psapi.dll", SetLastError = true)]
    static extern bool QueryWorkingSet(IntPtr process, IntPtr buffer, uint size);

    const uint PROCESS_QUERY_INFORMATION = 0x0400;
    const uint PROCESS_VM_READ = 0x0010;
    const int ERROR_BAD_LENGTH = 24;

    public class Result {
        public long PrivateBytes;
        public long WorkingSet;
        public long PssBytes;
        public long SharedBytes;
        public long Pages;
        public string Error;
    }

    public static Result Read(int pid, int pageSize) {
        var result = new Result();
        IntPtr handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);
        if (handle == IntPtr.Zero) {
            result.Error = "open_failed:" + Marshal.GetLastWin32Error();
            return result;
        }
        try {
            uint size = 1 << 16;
            IntPtr buffer = IntPtr.Zero;
            bool ok = false;
            for (int attempt = 0; attempt < 12; attempt++) {
                if (buffer != IntPtr.Zero) Marshal.FreeHGlobal(buffer);
                buffer = Marshal.AllocHGlobal((int)size);
                ok = QueryWorkingSet(handle, buffer, size);
                if (ok) break;
                int err = Marshal.GetLastWin32Error();
                if (err != ERROR_BAD_LENGTH) {
                    result.Error = "query_failed:" + err;
                    return result;
                }
                size *= 2;
                if (size > (1u << 27)) {
                    result.Error = "buffer_too_large";
                    return result;
                }
            }
            if (!ok || buffer == IntPtr.Zero) {
                result.Error = "query_failed";
                return result;
            }

            long entries = Marshal.ReadIntPtr(buffer).ToInt64();
            long entryBase = buffer.ToInt64() + IntPtr.Size;
            long pss = 0;
            long shared = 0;
            for (long i = 0; i < entries; i++) {
                long value = Marshal.ReadIntPtr(new IntPtr(entryBase + i * IntPtr.Size)).ToInt64();
                long shareCount = (value >> 5) & 0x7;
                bool isShared = ((value >> 8) & 0x1) != 0;
                if (shareCount < 1) shareCount = 1;
                if (isShared) {
                    shared += pageSize;
                    pss += pageSize / shareCount;
                } else {
                    pss += pageSize;
                }
            }
            result.Pages = entries;
            result.SharedBytes = shared;
            result.PssBytes = pss;
            return result;
        } finally {
            CloseHandle(handle);
        }
    }
}
"@

function Get-TreeIds([int]$root) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine
    $byParent = @{}
    foreach ($p in $all) {
        $key = [int]$p.ParentProcessId
        if (-not $byParent.ContainsKey($key)) { $byParent[$key] = New-Object System.Collections.Generic.List[object] }
        $byParent[$key].Add($p)
    }
    $seen = New-Object System.Collections.Generic.HashSet[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($root)
    [void]$seen.Add($root)
    $result = New-Object System.Collections.Generic.List[object]
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        $proc = $all | Where-Object { $_.ProcessId -eq $id } | Select-Object -First 1
        if ($proc) { $result.Add($proc) }
        if ($byParent.ContainsKey($id)) {
            foreach ($child in $byParent[$id]) {
                if (-not $seen.Contains([int]$child.ProcessId)) {
                    [void]$seen.Add([int]$child.ProcessId)
                    $queue.Enqueue([int]$child.ProcessId)
                }
            }
        }
    }
    return $result
}

function Get-Role($commandLine, $name) {
    if ($name -eq 'msedgewebview2.exe') {
        if ($commandLine -match '--type=([a-z-]+)') { return $matches[1] }
        return 'browser'
    }
    return $name
}

function Get-PrivateWorkingSet($ids) {
    $map = @{}
    try {
        $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess }
        foreach ($p in $perf) { $map[[int]$p.IDProcess] = [int64]$p.WorkingSetPrivate }
    } catch { }
    return $map
}

function Get-GpuMemory($ids) {
    $map = @{}
    try {
        $samples = (Get-Counter '\GPU Process Memory(*)\Total Committed' -ErrorAction Stop).CounterSamples
        foreach ($s in $samples) {
            if ($s.InstanceName -match 'pid_(\d+)') {
                $pidValue = [int]$matches[1]
                if ($ids -contains $pidValue -and $s.CookedValue -gt 0) {
                    $map[$pidValue] = [int64]$s.CookedValue
                }
            }
        }
    } catch { }
    return $map
}

$first = Get-TreeIds $RootId
$ids = @($first | ForEach-Object { [int]$_.ProcessId })
$cpu1 = @{}
foreach ($p in Get-Process -Id $ids -ErrorAction SilentlyContinue) {
    $cpu1[$p.Id] = $p.TotalProcessorTime.TotalSeconds
}
$t1 = [DateTime]::UtcNow
$privateWs1 = Get-PrivateWorkingSet $ids

Start-Sleep -Seconds $Seconds

$second = Get-TreeIds $RootId
$ids2 = @($second | ForEach-Object { [int]$_.ProcessId })
$t2 = [DateTime]::UtcNow
$elapsed = ($t2 - $t1).TotalSeconds
if ($elapsed -le 0) { $elapsed = $Seconds }

$privateWs2 = Get-PrivateWorkingSet $ids2
$gpu = Get-GpuMemory $ids2

$rows = @()
$totalWs = 0L
$totalPrivate = 0L
$totalPrivateWs = 0L
$totalPss = 0L
$totalShared = 0L
$cpuDelta = 0.0

foreach ($proc in $second) {
    $processId = [int]$proc.ProcessId
    $live = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if (-not $live) { continue }
    $pss = [PssReader]::Read($processId, $pageSize)
    $ws = $live.WorkingSet64
    $privateCommit = $live.PrivateMemorySize64
    $privateWs = if ($privateWs2.ContainsKey($processId)) { $privateWs2[$processId] } else { 0 }
    $gpuBytes = if ($gpu.ContainsKey($processId)) { $gpu[$processId] } else { 0 }
    $cpu = [double]$live.TotalProcessorTime.TotalSeconds
    $cpu0 = if ($cpu1.ContainsKey($processId)) { [double]$cpu1[$processId] } else { $cpu }
    $cpuDelta += [math]::Max(0, $cpu - $cpu0)
    $role = Get-Role $proc.CommandLine $proc.Name

    $totalWs += $ws
    $totalPrivate += $privateCommit
    $totalPrivateWs += $privateWs
    $totalPss += $pss.PssBytes
    $totalShared += $pss.SharedBytes

    $rows += [pscustomobject]@{
        pid = $processId
        role = $role
        ws_mb = [math]::Round($ws / 1MB, 1)
        private_ws_mb = [math]::Round($privateWs / 1MB, 1)
        private_commit_mb = [math]::Round($privateCommit / 1MB, 1)
        pss_mb = [math]::Round($pss.PssBytes / 1MB, 1)
        shared_mb = [math]::Round($pss.SharedBytes / 1MB, 1)
        gpu_mb = [math]::Round($gpuBytes / 1MB, 1)
        cpu_pct_one_core = if ($elapsed -gt 0) { [math]::Round(($cpu - $cpu0) / $elapsed * 100, 2) } else { 0 }
        pss_error = $pss.Error
    }
}

$cores = [Environment]::ProcessorCount
$cpuPctOneCore = (($rows | Measure-Object cpu_pct_one_core -Sum).Sum)
$summary = [pscustomobject]@{
    label = $Label
    sampled_at = $t2.ToString('o')
    seconds = [math]::Round($elapsed, 1)
    processes = $rows.Count
    ws_mb = [math]::Round($totalWs / 1MB, 1)
    private_ws_mb = [math]::Round($totalPrivateWs / 1MB, 1)
    private_commit_mb = [math]::Round($totalPrivate / 1MB, 1)
    pss_mb = [math]::Round($totalPss / 1MB, 1)
    shared_ws_mb = [math]::Round($totalShared / 1MB, 1)
    gpu_mb = [math]::Round((($rows | Measure-Object gpu_mb -Sum).Sum), 1)
    cpu_pct_one_core = [math]::Round($cpuPctOneCore, 2)
    cpu_pct_all_cores = [math]::Round($cpuPctOneCore / $cores, 2)
    logical_cores = $cores
    pss_note = "PSS divides shared pages by their 3-bit share count; pages shared by more than 7 processes are over-attributed."
}

if ($Json) {
    [pscustomobject]@{ summary = $summary; processes = $rows } | ConvertTo-Json -Depth 5
} else {
    $summary | Format-List
    "processes:"
    $rows | Sort-Object pss_mb -Descending | Format-Table pid, role, ws_mb, private_ws_mb, private_commit_mb, pss_mb, gpu_mb, cpu_pct_one_core, pss_error -AutoSize
}
