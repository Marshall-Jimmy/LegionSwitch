[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $versionMatch = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $versionMatch) {
        throw "Could not read version from Cargo.toml"
    }
    $version = $versionMatch.Matches[0].Groups[1].Value

    if (-not $SkipBuild) {
        Remove-Item Env:LEGIONSWITCH_PREVIEW -ErrorAction SilentlyContinue
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release failed"
        }
    }

    $sourceExe = Join-Path $root "target\release\LegionSwitch.exe"
    if (-not (Test-Path -LiteralPath $sourceExe)) {
        throw "Release executable not found: $sourceExe"
    }

    $dist = Join-Path $root "dist"
    $stage = Join-Path $dist "package"
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null

    Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $stage "LegionSwitch.exe") -Force
    Copy-Item -LiteralPath (Join-Path $root "README.md") -Destination $stage -Force
    Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $stage -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "install.ps1") -Destination $stage -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot "uninstall.ps1") -Destination $stage -Force
    Copy-Item -LiteralPath (Join-Path $root "docs") -Destination $stage -Recurse -Force
    "LegionSwitch $version portable package`r`nGenerated: $(Get-Date -Format o)" |
        Set-Content -LiteralPath (Join-Path $stage "VERSION.txt") -Encoding utf8

    $zip = Join-Path $dist "LegionSwitch-$version-portable.zip"
    Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal
    Remove-Item -LiteralPath $stage -Recurse -Force

    # Use a versioned standalone name so packaging also works while an older
    # installed/running LegionSwitch.exe is locked by Windows.
    $versionedExe = Join-Path $dist "LegionSwitch-$version.exe"
    Copy-Item -LiteralPath $sourceExe -Destination $versionedExe -Force
    $exeHash = (Get-FileHash -LiteralPath $versionedExe -Algorithm SHA256).Hash
    $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    @(
        "$exeHash  LegionSwitch-$version.exe",
        "$zipHash  LegionSwitch-$version-portable.zip"
    ) | Set-Content -LiteralPath (Join-Path $dist "SHA256SUMS.txt") -Encoding ascii

    Write-Host "Packaged LegionSwitch $version"
    Write-Host "  $zip"
    Write-Host "  $versionedExe"
    Write-Host "  $(Join-Path $dist 'SHA256SUMS.txt')"
}
finally {
    Pop-Location
}
