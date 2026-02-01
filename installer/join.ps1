#Requires -Version 5.1
<#
.SYNOPSIS
    Joins a viberails team on Windows.

.EXAMPLE
    $u="https://..."; irm https://get.viberails.io/join.ps1 | iex
#>

$ErrorActionPreference = "Stop"

$BaseUrl = "https://get.viberails.io"
$BinaryName = "viberails"

function Get-Architecture {
    $arch = [System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE")
    switch ($arch) {
        "AMD64" { return "x64" }
        "ARM64" {
            Write-Error "Windows ARM64 is not supported"
            exit 1
        }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }
}

function Invoke-Download {
    param(
        [string]$SourceUrl,
        [string]$Destination
    )

    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        $webClient = New-Object System.Net.WebClient
        $webClient.DownloadFile($SourceUrl, $Destination)
    }
    catch {
        Write-Error "Failed to download from $SourceUrl : $_"
        exit 1
    }
}

function Get-Binary {
    $arch = Get-Architecture
    $artifactName = "$BinaryName-windows-$arch.exe"
    $downloadUrl = "$BaseUrl/$artifactName"

    Write-Host "Detected: windows $arch"
    Write-Host "Downloading $artifactName..."

    $tmpDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    $tmpFile = Join-Path $tmpDir "$artifactName"

    Invoke-Download -SourceUrl $downloadUrl -Destination $tmpFile

    Write-Host "Successfully downloaded $BinaryName"

    return @{
        TmpDir = $tmpDir
        TmpFile = $tmpFile
    }
}

# Main
if (-not $u) {
    Write-Error "Variable `$u is required"
    Write-Host "Usage: `$u=`"<team-url>`"; irm https://get.viberails.io/join.ps1 | iex"
    exit 1
}

$download = Get-Binary

try {
    & $download.TmpFile -V
    & $download.TmpFile join $u
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $download.TmpFile install
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally {
    Remove-Item -Recurse -Force $download.TmpDir -ErrorAction SilentlyContinue
}
