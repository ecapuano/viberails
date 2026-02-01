#Requires -Version 5.1
<#
.SYNOPSIS
    Installs, upgrades, or uninstalls viberails on Windows.

.PARAMETER Command
    The command to run: install (default), join-team, uninstall, or upgrade.

.PARAMETER Url
    The URL to use with join-team command.

.EXAMPLE
    .\install.ps1
    Installs viberails.

.EXAMPLE
    .\install.ps1 -Command upgrade
    Upgrades viberails to the latest version.

.EXAMPLE
    .\install.ps1 -Command join-team -Url "https://example.com/team/abc123"
    Joins a team using the provided URL.
#>

param(
    [ValidateSet("install", "join-team", "uninstall", "upgrade")]
    [string]$Command = "install",
    [string]$Url
)

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

function Invoke-Install {
    $download = Get-Binary

    try {
        # Display version
        & $download.TmpFile -V

        # Run init-team
        & $download.TmpFile init-team
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

        # Run install
        & $download.TmpFile install
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    finally {
        # Cleanup
        Remove-Item -Recurse -Force $download.TmpDir -ErrorAction SilentlyContinue
    }
}

function Invoke-JoinTeam {
    param([string]$TeamUrl)

    if (-not $TeamUrl) {
        Write-Error "join-team requires a URL argument"
        Write-Host "Usage: .\install.ps1 -Command join-team -Url <url>"
        exit 1
    }

    $download = Get-Binary

    try {
        # Display version
        & $download.TmpFile -V

        # Run join-team
        & $download.TmpFile join $TeamUrl
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

        # Run install
        & $download.TmpFile install
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    finally {
        # Cleanup
        Remove-Item -Recurse -Force $download.TmpDir -ErrorAction SilentlyContinue
    }
}

function Invoke-Uninstall {
    $download = Get-Binary

    try {
        # Display version
        & $download.TmpFile -V

        # Run uninstall
        & $download.TmpFile uninstall
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    finally {
        # Cleanup
        Remove-Item -Recurse -Force $download.TmpDir -ErrorAction SilentlyContinue
    }
}

function Invoke-Upgrade {
    $download = Get-Binary

    try {
        # Display version
        & $download.TmpFile -V

        # Run upgrade
        & $download.TmpFile upgrade
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    finally {
        # Cleanup
        Remove-Item -Recurse -Force $download.TmpDir -ErrorAction SilentlyContinue
    }
}

# Main
switch ($Command) {
    "install" {
        Invoke-Install
    }
    "join-team" {
        Invoke-JoinTeam -TeamUrl $Url
    }
    "uninstall" {
        Invoke-Uninstall
    }
    "upgrade" {
        Invoke-Upgrade
    }
}
