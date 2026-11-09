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

-Download keeps the installer in that folder (CI caches it there); a copy already there with
the pinned SHA-256 is used without downloading.
#>
param(
    [string]$Version = "1.28.7",
    [string]$Sha256 = "032fc6062b8539838fc8da22589cb9b24c5d820baa7f8cc160af9ea08395badf",
    # Where the installer is kept; empty: the temp folder, deleted after the install.
    [string]$Download = ""
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
    $keep = [bool]$Download
    $folder = if ($keep) { (New-Item -ItemType Directory -Force $Download).FullName } else { [IO.Path]::GetTempPath() }
    $setup = Join-Path $folder $file
    $url = "https://gstreamer.freedesktop.org/data/pkg/windows/$Version/msvc/$file"
    $done = (Test-Path $setup) -and (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLower() -eq $Sha256
    if ($done) { Write-Host "GStreamer installer from $folder" }
    elseif (Test-Path $setup) { Remove-Item $setup }
    # The server sometimes ends a transfer after a few MB ("response ended prematurely",
    # 2026-11-09). curl.exe (part of Windows since 10 1803) picks up where it stopped (`-C -`,
    # a Range request); the SHA-256 below checks the joined file.
    foreach ($attempt in 1..60) {
        if ($done) { break }
        & curl.exe --fail --silent --show-error --location --continue-at - --max-time 300 `
            --output $setup $url
        if ($LASTEXITCODE -eq 0) { $done = $true; break }
        Write-Host "GStreamer download stopped (curl $LASTEXITCODE), attempt $attempt - resuming"
        Start-Sleep -Seconds 2
    }
    if (-not $done) { throw "could not download $url" }
    $hash = (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLower()
    if ($hash -ne $Sha256) { throw "$file has SHA-256 $hash, expected $Sha256" }
    # `devel`: runtime and development files (headers, import libraries, pkg-config).
    $p = Start-Process -FilePath $setup -Wait -PassThru -ArgumentList `
        "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/TYPE=devel"
    if ($p.ExitCode) { throw "the GStreamer installer failed ($($p.ExitCode))" }
    if (-not $keep) { Remove-Item $setup }
    if (-not (Test-Path $installed)) { throw "GStreamer is not in $root after the installer" }
    Write-Host "GStreamer $Version installed: $root"
}
if ($env:GITHUB_ENV) { "GSTREAMER_ROOT=$root" | Out-File -Append -Encoding utf8 $env:GITHUB_ENV }
$root
