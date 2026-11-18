#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Uninstall LegionSwitch.

.DESCRIPTION
    Stops LegionSwitch, removes the logon scheduled task, Start Menu shortcut,
    "Apps & features" entry and the installation directory.
    Pass -KeepSettings to preserve %APPDATA%\LegionSwitch\config.ini.
#>
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:ProgramFiles "LegionSwitch"),
    [switch]$KeepSettings
)

$ErrorActionPreference = "SilentlyContinue"

Get-Process -Name LegionSwitch -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

schtasks.exe /Delete /TN "LegionSwitch" /F | Out-Null

$startMenu = Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs"
Remove-Item -LiteralPath (Join-Path $startMenu "LegionSwitch.lnk") -Force

Remove-Item -Path "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LegionSwitch" -Recurse -Force

if (-not $KeepSettings) {
    Remove-Item -LiteralPath (Join-Path $env:APPDATA "LegionSwitch") -Recurse -Force
}

if (Test-Path -LiteralPath $InstallDir) {
    Remove-Item -LiteralPath $InstallDir -Recurse -Force
}

Write-Host "LegionSwitch removed."
if ($KeepSettings) {
    Write-Host "User settings were kept in %APPDATA%\LegionSwitch."
}
