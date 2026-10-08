# Install the latest (or a given) Tokit release on Windows:
#   irm https://raw.githubusercontent.com/AlphaVIE/tokit/main/scripts/install.ps1 | iex
# Installs tok.exe into $env:TOK_INSTALL (default ~\.tok\bin) after verifying its SHA-256
# and adds that directory to the user PATH.
param([string]$Version = "latest")
$ErrorActionPreference = "Stop"

$repo = "AlphaVIE/tokit"
$target = "x86_64-pc-windows-msvc"
$archive = "tok-$target.zip"
$dest = if ($env:TOK_INSTALL) { $env:TOK_INSTALL } else { Join-Path $HOME ".tok\bin" }
$base = if ($Version -eq "latest") {
    "https://github.com/$repo/releases/latest/download"
} else {
    "https://github.com/$repo/releases/download/$Version"
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("tok-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Invoke-WebRequest "$base/$archive" -OutFile (Join-Path $work $archive) -UseBasicParsing
    Invoke-WebRequest "$base/SHA256SUMS" -OutFile (Join-Path $work "SHA256SUMS") -UseBasicParsing
    $line = Get-Content (Join-Path $work "SHA256SUMS") | Where-Object { $_ -match " $([regex]::Escape($archive))$" }
    if (-not $line) { throw "no checksum for $archive" }
    $expected = ($line -split ' ')[0]
    $actual = (Get-FileHash (Join-Path $work $archive) -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw "checksum mismatch for $archive" }
    Expand-Archive (Join-Path $work $archive) -DestinationPath $work
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Copy-Item (Join-Path $work "tok-$target\tok.exe") (Join-Path $dest "tok.exe") -Force
} finally {
    Remove-Item -Recurse -Force $work
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not ($userPath -split ';' | Where-Object { $_ -eq $dest })) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$dest", "User")
    Write-Host "added $dest to your user PATH (open a new terminal)"
}
Write-Host "installed tok to $dest\tok.exe"
Write-Host "tok run works now; tok build and tok run --native also need Rust (https://rustup.rs)."
