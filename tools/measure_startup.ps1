#Requires -Version 7
<#
.SYNOPSIS
Measures Cerno's start-up: from the moment Windows creates the process to the first photo.

.DESCRIPTION
Starts cerno.exe with CERNO_DATA_DIR on its own data folder - give it a COPY of an index, never
the live one - reads the `start-up:` log lines from the redirected stderr and prints per run,
in ms since the process was created:

  main   when `main` began: loading the exe and its DLLs (not visible to Cerno's own timer)
  ready  "app state ready"   (index, folder scan, preload, the first decode started)
  frame  "first frame"       (window and GPU set up)
  photo  "first photo drawn"

Each run is ended once its first photo is drawn (the data folder is a copy, nothing is lost).

Cold and warm: Windows keeps files it read recently - the exe, its DLLs, the index - in the
standby list, so only the first start after a reboot or sign-in is cold. To repeat a cold run
without rebooting, empty the standby list first (Sysinternals RAMMap, as administrator:
Empty > Empty Standby List) and run with -Runs 1. Later runs are warm.

.EXAMPLE
pwsh tools/measure_startup.ps1 -Exe dist\windows\Cerno\cerno.exe -Folder D:\Photos -DataDir $env:TEMP\cerno-measure
#>
param(
    [Parameter(Mandatory)] [string]$Exe,
    # Opened as Cerno's start argument (a folder or a photo).
    [Parameter(Mandatory)] [string]$Folder,
    # Cerno's data folder for the runs; put a copy of cerno.db in it for a realistic preload.
    [Parameter(Mandatory)] [string]$DataDir,
    [int]$Runs = 5,
    [int]$TimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$live = Join-Path $env:LOCALAPPDATA "Cerno\data"
if ((Resolve-Path $DataDir).Path.TrimEnd('\') -ieq $live) {
    throw "DataDir is the live data folder - use a copy"
}
$exePath = (Resolve-Path $Exe).Path
$logDir = Join-Path $DataDir "measure-logs"
New-Item -ItemType Directory -Force $logDir | Out-Null

# `[2026-10-10T11:04:12.345Z INFO  cerno::app] start-up: first frame after 230 ms`
function Read-Stamp([string]$line) {
    if ($line -match '^\[(\S+Z)') {
        return [DateTimeOffset]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    }
    return $null
}

$results = @()
for ($run = 1; $run -le $Runs; $run++) {
    $log = Join-Path $logDir "run-$run.log"
    $env:CERNO_DATA_DIR = (Resolve-Path $DataDir).Path
    $env:RUST_LOG = "info"
    $process = Start-Process -FilePath $exePath -ArgumentList "`"$Folder`"" -PassThru `
        -RedirectStandardError $log
    $created = [DateTimeOffset]$process.StartTime.ToUniversalTime()
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $done = $false
    while (-not $done -and (Get-Date) -lt $deadline -and -not $process.HasExited) {
        Start-Sleep -Milliseconds 100
        if (Test-Path $log) {
            $done = [bool](Select-String -Path $log -Pattern "start-up: first photo drawn" -Quiet)
        }
    }
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    $process.WaitForExit()
    # Its children (ExifTool, the frame helpers) too: one started just before the kill would
    # stay, and keep this script's output pipe open. A child keeps its parent's id.
    Start-Sleep -Milliseconds 300
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$($process.Id)" |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    if (-not $done) {
        Write-Warning "run ${run}: no first photo within $TimeoutSeconds s (log: $log)"
        continue
    }

    $row = [ordered]@{ run = $run; main = $null; ready = $null; frame = $null; photo = $null }
    foreach ($line in Get-Content $log) {
        $stamp = Read-Stamp $line
        if (-not $stamp) { continue }
        $at = [int]($stamp - $created).TotalMilliseconds
        if ($line -match 'start-up: app state ready after (\d+) ms') {
            $row.ready = $at
            $row.main = $at - [int]$Matches[1]
        } elseif ($line -match 'start-up: first frame after') {
            $row.frame = $at
        } elseif ($line -match 'start-up: first photo drawn after') {
            $row.photo = $at
        }
    }
    $results += [pscustomobject]$row
    Start-Sleep -Milliseconds 500
}

$results | Format-Table -AutoSize
if ($results.Count -gt 1) {
    $warm = $results | Select-Object -Skip 1
    $median = {
        param($values)
        $sorted = @($values | Sort-Object)
        $sorted[[int][Math]::Floor(($sorted.Count - 1) / 2)]
    }
    "median of runs 2..{0}: main {1} ms, ready {2} ms, frame {3} ms, photo {4} ms" -f `
        $results.Count, (& $median $warm.main), (& $median $warm.ready), `
        (& $median $warm.frame), (& $median $warm.photo)
}
