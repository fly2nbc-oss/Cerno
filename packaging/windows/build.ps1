#Requires -Version 7
<#
.SYNOPSIS
Packs a Windows release build into dist\windows.

.DESCRIPTION
Run after `cargo build --release --features heic`. Produces

  Cerno\                           the portable folder: exe, DLLs, licenses
  cerno_<version>_x64-portable.zip that folder as a zip
  cerno_<version>_x64-setup.exe    NSIS installer of the same folder (cargo-packager)

The VC++ runtime DLLs that cerno.exe, heif.dll and libde265.dll import are copied
from Visual Studio's redist folder (app-local deployment), so neither the zip nor
the installer needs the VC++ Redistributable on the target machine. Every import
of every DLL in the folder is then checked: it must be in the folder or part of
Windows.
#>
param(
    # The cargo output directory with cerno.exe, the DLLs and licenses\.
    [string]$Target = "target\release",
    # Microsoft.VC14x.CRT folder; found through vswhere when not given.
    [string]$CrtDir,
    # Only the folder and the zip, no installer.
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

$version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
$dist = "dist\windows"
$stage = "$dist\Cerno"
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $stage | Out-Null

# --- The build's files ------------------------------------------------------
foreach ($name in "cerno.exe", "DirectML.dll", "heif.dll", "libde265.dll") {
    $src = Join-Path $Target $name
    if (-not (Test-Path $src)) {
        throw "$src is missing - run: cargo build --release --features heic"
    }
    # File.Copy follows symlinks: ort's copy-dylibs may link DirectML.dll into its cache.
    [System.IO.File]::Copy((Resolve-Path $src).Path, (Join-Path (Resolve-Path $stage).Path $name))
}
if ((Get-Item "$stage\DirectML.dll").Length -lt 1MB) {
    throw "DirectML.dll is suspiciously small - a link instead of the library?"
}
Copy-Item -Recurse (Join-Path $Target "licenses") "$stage\licenses"

# --- Visual Studio tools ----------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw "no Visual Studio with the C++ tools found" }
$dumpbin = Get-ChildItem "$vs\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" |
    Sort-Object { [version]$_.Directory.Parent.Parent.Parent.Name } | Select-Object -Last 1
if (-not $dumpbin) { throw "dumpbin.exe not found under $vs" }
if (-not $CrtDir) {
    $crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Parent.Parent.Name -match '^\d+(\.\d+)+$' } |
        Sort-Object { [version]$_.Parent.Parent.Name }, Name | Select-Object -Last 1
    if (-not $crt) { throw "no VC++ redist folder under $vs\VC\Redist\MSVC - pass -CrtDir" }
    $CrtDir = $crt.FullName
}
Write-Host "dumpbin: $($dumpbin.FullName)"
Write-Host "VC++ runtime: $CrtDir"

function Get-Imports([string]$File) {
    $lines = & $dumpbin.FullName /nologo /dependents $File
    if ($LASTEXITCODE) { throw "dumpbin failed on $File" }
    $lines | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^[\w.-]+\.dll$' }
}

function Get-Binaries { Get-ChildItem $stage -File | Where-Object Extension -in ".exe", ".dll" }

# --- App-local VC++ runtime ----------------------------------------------------
# Copy what is imported until nothing new is (msvcp140 itself imports vcruntime140).
do {
    $added = $false
    foreach ($binary in Get-Binaries) {
        foreach ($dll in Get-Imports $binary.FullName) {
            if (Test-Path "$stage\$dll") { continue }
            if (Test-Path "$CrtDir\$dll") {
                Copy-Item "$CrtDir\$dll" $stage
                $added = $true
            }
        }
    }
} while ($added)

# --- Every import is in the folder or part of Windows -------------------------------
# The VC++ runtime is installed on most machines (and on CI runners) but is not part
# of Windows, so finding it in System32 does not count.
$vcRuntime = '^(msvcp|vcruntime|concrt|vccorlib|vcomp)\d'
$missing = foreach ($binary in Get-Binaries) {
    foreach ($dll in Get-Imports $binary.FullName) {
        if (Test-Path "$stage\$dll") { continue }
        if ($dll -match '^(api|ext)-ms-win-') { continue } # API sets, resolved by Windows
        if ($dll -notmatch $vcRuntime -and (Test-Path "$env:SystemRoot\System32\$dll")) { continue }
        "$($binary.Name) -> $dll"
    }
}
if ($missing) {
    throw "imports that are neither in the package nor part of Windows:`n  $($missing -join "`n  ")"
}
Get-ChildItem $stage -Recurse -File | ForEach-Object {
    "{0,10:N0}  {1}" -f $_.Length, $_.FullName.Substring((Resolve-Path $stage).Path.Length + 1)
}

# --- Zip and installer ----------------------------------------------------------
Compress-Archive -Path $stage -DestinationPath "$dist\cerno_${version}_x64-portable.zip"
if (-not $NoInstaller) {
    # Folder, resources, icon and languages are configured in Cargo.toml.
    cargo packager --release --formats nsis
    if ($LASTEXITCODE) { throw "cargo packager failed" }
}
Get-ChildItem $dist -File | ForEach-Object {
    "{0}  {1,12:N0}  {2}" -f (Get-FileHash $_.FullName SHA256).Hash.ToLower(), $_.Length, $_.Name
}
