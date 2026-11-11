param([ValidateSet('debug', 'release')][string]$Configuration = 'release', [string]$Version = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $Version) { $Version = (Get-Content VERSION -Raw).Trim() }
    $versionText = python scripts/release_version.py $Version
    if ($LASTEXITCODE -ne 0) { throw 'Invalid package version.' }
    $env:KRILL_VERSION = $versionText.Trim()
    $arguments = @('build', '--locked', '--package', 'krill-desktop')
    if ($Configuration -eq 'release') { $arguments += '--release' }
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Rust client build failed.' }
    $package = Join-Path $projectRoot "dist/Krill.Desktop-$versionText-win-x64"
    if (Test-Path -LiteralPath $package) { Remove-Item -LiteralPath $package -Recurse -Force }
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item "target/$Configuration/KrillMac.exe" (Join-Path $package 'Krill.Desktop.exe') -Force
    # The marker is generated as .NET metadata by Rust. It contains no C# code,
    # has no runtime dependencies, and is never loaded by the native client.
    cargo run --locked --package krill-package -- $versionText $package
    if ($LASTEXITCODE -ne 0) { throw 'Compatibility metadata generation failed.' }
    $legacyVersion = [System.Reflection.AssemblyName]::GetAssemblyName((Join-Path $package 'Krill.Desktop.dll')).Version.ToString(3)
    if ($legacyVersion -ne $versionText) { throw 'Legacy updater compatibility check failed.' }
    Copy-Item README.md $package -Force
    if ($Configuration -eq 'release') {
        $archive = Join-Path $projectRoot "dist/Krill.Desktop-$versionText-win-x64.zip"
        Compress-Archive -Path (Join-Path $package '*') -DestinationPath $archive -Force
        Write-Output $archive
    }
    Write-Output $package
} finally {
    Remove-Item Env:KRILL_VERSION -ErrorAction SilentlyContinue
    Pop-Location
}
