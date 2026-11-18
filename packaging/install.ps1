#Requires -RunAsAdministrator
<#
.SYNOPSIS
    LegionSwitch 1.0.0 installer.

.DESCRIPTION
    Copies LegionSwitch.exe to a permanent folder, creates a Start Menu
    shortcut and registers an entry in Windows "Apps & features".
    The app itself still asks for UAC when launched; this script just
    performs the one-time installation.

.EXAMPLE
    .\install.ps1
    .\install.ps1 -EnableAutostart
    .\install.ps1 -InstallDir "D:\Tools\LegionSwitch"
#>
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:ProgramFiles "LegionSwitch"),
    [switch]$EnableAutostart,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

$sourceDir = $PSScriptRoot
$sourceExe = Join-Path $sourceDir "LegionSwitch.exe"
if (-not (Test-Path -LiteralPath $sourceExe)) {
    throw "LegionSwitch.exe not found next to install.ps1. Extract the portable ZIP completely first."
}

$existing = Get-Process -Name LegionSwitch -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Stopping running LegionSwitch..."
    $existing | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400
}

Write-Host "Installing to $InstallDir"
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

$targetExe = Join-Path $InstallDir "LegionSwitch.exe"
Copy-Item -LiteralPath $sourceExe -Destination $targetExe -Force
foreach ($name in @("README.md", "LICENSE", "uninstall.ps1")) {
    $source = Join-Path $sourceDir $name
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $InstallDir $name) -Force
    }
}

# Start Menu shortcut.
$startMenu = Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs"
New-Item -ItemType Directory -Path $startMenu -Force | Out-Null
$shortcutPath = Join-Path $startMenu "LegionSwitch.lnk"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $targetExe
$shortcut.WorkingDirectory = $InstallDir
$shortcut.IconLocation = "$targetExe,0"
$shortcut.Description = "LegionSwitch - Lenovo Legion performance modes"
$shortcut.Save()

# Windows "Apps & features" entry.
$version = (Get-Item -LiteralPath $targetExe).VersionInfo.FileVersion
if ([string]::IsNullOrWhiteSpace($version)) {
    $version = "1.0.0"
}
$uninstallKey = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LegionSwitch"
New-Item -Path $uninstallKey -Force | Out-Null
$uninstallScript = Join-Path $InstallDir "uninstall.ps1"
$properties = @{
    DisplayName = "LegionSwitch"
    DisplayVersion = $version
    Publisher = "jimmma"
    InstallLocation = $InstallDir
    DisplayIcon = $targetExe
    UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstallScript`""
    NoModify = 1
    NoRepair = 1
}
foreach ($key in $properties.Keys) {
    $propertyType = if ($key -in @("NoModify", "NoRepair")) { "DWord" } else { "String" }
    New-ItemProperty -Path $uninstallKey -Name $key -Value $properties[$key] -PropertyType $propertyType -Force | Out-Null
}

if ($EnableAutostart) {
    Write-Host "Enabling logon autostart (scheduled task)..."
    $process = Start-Process -FilePath $targetExe -ArgumentList "startup-install" -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -ne 0) {
        Write-Warning "The autostart task could not be created (exit $($process.ExitCode)). You can enable it later from the tray menu."
    }
}

Write-Host "LegionSwitch installed."
Write-Host "  Start Menu: $shortcutPath"
Write-Host "  App:        $targetExe"
if (-not $NoLaunch) {
    Start-Process -FilePath $targetExe
}
