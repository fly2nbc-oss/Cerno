#Requires -Version 7
<#
.SYNOPSIS
Measures how long Cerno takes to end once its window is asked to close.

.DESCRIPTION
Starts cerno.exe with CERNO_DATA_DIR on its own data folder - give it a COPY of an index, never
the live one -, waits for the first photo, then -After seconds more (the analysis loads its
models ~3 s after the photo, the prediction trains 2 s after the start), optionally posts keys
(-Keys, virtual key codes: 0x20 plays a video opened with -Open), and posts WM_CLOSE to its
window. Prints per run, in ms after WM_CLOSE: when the window and the process were gone, the
exit code, and the `exit:` lines of the log (and the last training before it).

eframe drops the app before the window, so the window and the process go together: the time
is what the user sees as a frozen window.

Open a folder only to read it: nothing here sets a mark, but the analysis fills the index in
-DataDir. Never post keys that rate, delete or edit to a folder of real photos.

.EXAMPLE
pwsh tools/measure_exit.ps1 -Exe target\release\cerno.exe -Open D:\Photos -DataDir $env:TEMP\cerno-exit -After 10 -Runs 3
#>
param(
    [Parameter(Mandatory)] [string]$Exe,
    # Cerno's start argument (a folder, a photo or a video).
    [Parameter(Mandatory)] [string]$Open,
    # Cerno's data folder for the runs; an empty one makes the analysis run.
    [Parameter(Mandatory)] [string]$DataDir,
    [double]$After = 5,
    [int[]]$Keys = @(),
    [double]$AfterKeys = 2,
    [int]$Runs = 1
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$live = Join-Path $env:LOCALAPPDATA "Cerno\data"
if ((Resolve-Path $DataDir).Path.TrimEnd('\') -ieq $live) {
    throw "DataDir is the live data folder - use a copy"
}

Add-Type -Namespace CernoExit -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
public delegate bool EnumProc(IntPtr h, IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
// The window whose title names Cerno - not wgpu's untitled helper window.
public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
        uint p; GetWindowThreadProcessId(h, out p);
        if (p != pid) return true;
        var title = new System.Text.StringBuilder(256); GetWindowText(h, title, 256);
        if (title.ToString().Contains("Cerno")) { found = h; return false; }
        return true;
    }, IntPtr.Zero);
    return found;
}
'@
$win = [CernoExit.Win]

# `[2026-10-13T19:30:48.868Z INFO  cerno::analysis] exit: analysis stopped after 264 ms`
function Read-Stamp([string]$line) {
    if ($line -match '^\[(\S+Z)') {
        return [DateTimeOffset]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    }
    return $null
}

$exePath = (Resolve-Path $Exe).Path
$logDir = Join-Path $DataDir "exit-logs"
New-Item -ItemType Directory -Force $logDir | Out-Null

for ($run = 1; $run -le $Runs; $run++) {
    $log = Join-Path $logDir "run-$run.log"
    $env:CERNO_DATA_DIR = (Resolve-Path $DataDir).Path
    $env:RUST_LOG = "info"
    $process = Start-Process -FilePath $exePath -ArgumentList "`"$Open`"" -PassThru `
        -RedirectStandardError $log
    $deadline = (Get-Date).AddSeconds(60)
    $shown = $false
    while (-not $shown -and (Get-Date) -lt $deadline -and -not $process.HasExited) {
        Start-Sleep -Milliseconds 100
        $shown = (Test-Path $log) -and
            [bool](Select-String -Path $log -Pattern "start-up: first photo drawn" -Quiet)
    }
    if (-not $shown) {
        Write-Warning "run ${run}: no first photo within 60 s (log: $log)"
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
        continue
    }
    Start-Sleep -Milliseconds ([int]($After * 1000))
    $hwnd = $win::Find([uint32]$process.Id)
    if ($hwnd -eq [IntPtr]::Zero) { throw "run ${run}: no window titled Cerno" }
    foreach ($key in $Keys) {
        $scan = [int64]$win::MapVirtualKey([uint32]$key, 0)
        [void]$win::PostMessage($hwnd, 0x0100, [IntPtr]$key, [IntPtr](1 -bor ($scan -shl 16)))
        Start-Sleep -Milliseconds 50
        $up = 1 -bor ($scan -shl 16) -bor (3L -shl 30)
        [void]$win::PostMessage($hwnd, 0x0101, [IntPtr]$key, [IntPtr]$up)
        Start-Sleep -Milliseconds 100
    }
    if ($Keys.Count) { Start-Sleep -Milliseconds ([int]($AfterKeys * 1000)) }

    $asked = [DateTimeOffset]::UtcNow
    [void]$win::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)   # WM_CLOSE
    $windowGone = $null
    while (-not $process.HasExited) {
        if ($null -eq $windowGone -and -not $win::IsWindow($hwnd)) {
            $windowGone = [DateTimeOffset]::UtcNow
        }
        if (([DateTimeOffset]::UtcNow - $asked).TotalSeconds -gt 60) {
            Write-Warning "run ${run}: still running 60 s after WM_CLOSE"
            Stop-Process -Id $process.Id -Force
            break
        }
        Start-Sleep -Milliseconds 5
    }
    $ended = [DateTimeOffset]::UtcNow
    if ($null -eq $windowGone) { $windowGone = $ended }
    $process.WaitForExit()
    # Its children (ExifTool, the frame helpers) too, should one outlive it.
    Start-Sleep -Milliseconds 300
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$($process.Id)" |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }

    "run ${run}: window gone after {0} ms, process after {1} ms, exit code {2}" -f `
        [int]($windowGone - $asked).TotalMilliseconds, [int]($ended - $asked).TotalMilliseconds, `
        $process.ExitCode
    foreach ($line in Get-Content $log) {
        if ($line -notmatch 'exit:|taste model|ERROR') { continue }
        $stamp = Read-Stamp $line
        if (-not $stamp) { continue }
        $at = [int]($stamp - $asked).TotalMilliseconds
        if ($at -lt -5000) { continue }
        "  {0,6} ms  {1}" -f $at, ($line -replace '^\[\S+ \w+\s+', '')
    }
    Start-Sleep -Milliseconds 500
}
