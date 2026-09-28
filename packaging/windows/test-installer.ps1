#Requires -Version 7
<#
.SYNOPSIS
Installs, starts and uninstalls the NSIS installer silently (CI only).

.DESCRIPTION
Checks that the per-user install lands in %LOCALAPPDATA%\Cerno with every file
of the portable folder, that the installed exe starts and draws a photo, and
that uninstalling removes the program but keeps Cerno's data folder
(%LOCALAPPDATA%\Cerno\data - the index with the user's ratings history).

Do not run this on a machine where Cerno is installed or used: it installs over
and uninstalls that copy.
#>
param(
    [Parameter(Mandatory)][string]$Setup,
    # A photo to open; the test passes when Cerno logs that it drew it.
    [Parameter(Mandatory)][string]$Photo
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$dir = "$env:LOCALAPPDATA\Cerno"
$stage = "$PSScriptRoot\..\..\dist\windows\Cerno"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cerno"
$marker = "$dir\data\installer-test-marker"

# Stands in for the user's index, which must survive the uninstall.
New-Item -ItemType File -Force $marker | Out-Null

$p = Start-Process $Setup -ArgumentList "/S" -Wait -PassThru
if ($p.ExitCode) { throw "installer exited with $($p.ExitCode)" }

$expected = Get-ChildItem $stage -Recurse -File |
    ForEach-Object { $_.FullName.Substring((Resolve-Path $stage).Path.Length + 1) }
$absent = $expected | Where-Object { -not (Test-Path "$dir\$_") }
if ($absent) { throw "not installed: $($absent -join ', ')" }
if (-not (Test-Path "$dir\uninstall.exe")) { throw "no uninstall.exe" }
if (-not (Test-Path $uninstallKey)) { throw "no entry in Apps & features" }
Write-Host "installed: $($expected.Count) files in $dir"

# The installed exe starts, renders and decodes. It runs until it is stopped, so
# wait for the log line of the first photo, then close it.
$log = "$env:RUNNER_TEMP\cerno-start.log"
$env:RUST_LOG = "info"
$cerno = Start-Process "$dir\cerno.exe" -ArgumentList "`"$Photo`"" -PassThru -RedirectStandardError $log
$drawn = $false
foreach ($i in 1..60) {
    Start-Sleep -Milliseconds 500
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern "start-up: first photo drawn" -Quiet)) {
        $drawn = $true
        break
    }
    if ($cerno.HasExited) { break }
}
$exited = $cerno.HasExited
if (-not $exited) { Stop-Process -Id $cerno.Id -Force; $cerno.WaitForExit() }
Get-Content $log -ErrorAction SilentlyContinue | Select-Object -Last 40
if (-not $drawn) {
    throw "Cerno did not draw the photo (exited: $exited, code: $(if ($exited) { $cerno.ExitCode }))"
}

# `_?=` runs the uninstaller in place and waits for it (otherwise it copies itself to
# %TEMP% and returns at once); it then cannot delete itself, so uninstall.exe stays.
$p = Start-Process "$dir\uninstall.exe" -ArgumentList "/S", "_?=$dir" -Wait -PassThru
if ($p.ExitCode) { throw "uninstaller exited with $($p.ExitCode)" }
$left = $expected | Where-Object { Test-Path "$dir\$_" }
if ($left) { throw "left behind after uninstall: $($left -join ', ')" }
if (Test-Path $uninstallKey) { throw "Apps & features entry left behind" }
if (-not (Test-Path $marker)) { throw "the uninstaller deleted Cerno's data folder" }
Remove-Item -Force $marker, "$dir\uninstall.exe"
Write-Host "uninstalled; the data folder was kept"
