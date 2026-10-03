#Requires -Version 7
<#
.SYNOPSIS
Installs the GStreamer that `--features video` builds against and that build.ps1 ships.

.DESCRIPTION
The official MSVC build (runtime and development files in one Inno Setup installer since
1.28), for the current user – no admin rights – into
%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64. The installer is checked against the
pinned SHA-256 first. Nothing happens when that version is installed already.

Prints the root folder. In GitHub Actions it is also written to $GITHUB_ENV as
GSTREAMER_ROOT; each step sets PKG_CONFIG (and PATH, where it runs the player) itself, so
the package's self-test never finds this installation by accident.

Developers: run it once, then before `cargo build --features video`:

    $gst = "$env:LOCALAPPDATA\Programs\gstreamer\1.0\msvc_x86_64"
    $env:PKG_CONFIG = "$gst\bin\pkg-config.exe"; $env:PKG_CONFIG_PATH = "$gst\lib\pkgconfig"
    $env:PATH = "$gst\bin;$env:PATH"   # for cargo run / cargo test
#>
param(
    [string]$Version = "1.28.7",
    [string]$Sha256 = "032fc6062b8539838fc8da22589cb9b24c5d820baa7f8cc160af9ea08395badf"
)

$ErrorActionPreference = "Stop"
$root = "$env:LOCALAPPDATA\Programs\gstreamer\1.0\msvc_x86_64"
$installed = "$root\bin\gstreamer-1.0-0.dll"
$current = if (Test-Path "$root\bin\gst-inspect-1.0.exe") {
    (& "$root\bin\gst-inspect-1.0.exe" --version | Select-Object -First 1) -replace '.*version ', ''
}
if ((Test-Path $installed) -and $current -eq $Version) {
    Write-Host "GStreamer $Version is installed: $root"
} else {
    $file = "gstreamer-1.0-msvc-x86_64-$Version.exe"
    $setup = Join-Path ([IO.Path]::GetTempPath()) $file
    $ProgressPreference = "SilentlyContinue"
    Invoke-WebRequest -UseBasicParsing "https://gstreamer.freedesktop.org/data/pkg/windows/$Version/msvc/$file" -OutFile $setup
    $hash = (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLower()
    if ($hash -ne $Sha256) { throw "$file has SHA-256 $hash, expected $Sha256" }
    # `devel`: runtime and development files (headers, import libraries, pkg-config).
    $p = Start-Process -FilePath $setup -Wait -PassThru -ArgumentList `
        "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/TYPE=devel"
    if ($p.ExitCode) { throw "the GStreamer installer failed ($($p.ExitCode))" }
    Remove-Item $setup
    if (-not (Test-Path $installed)) { throw "GStreamer is not in $root after the installer" }
    Write-Host "GStreamer $Version installed: $root"
}
if ($env:GITHUB_ENV) { "GSTREAMER_ROOT=$root" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV }
$root
