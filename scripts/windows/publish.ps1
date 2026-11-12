param([switch]$FrameworkDependent, [string]$Version)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$projectFile = Join-Path $projectRoot 'apps\windows\src\Krill.Desktop\Krill.Desktop.csproj'
$flavorSuffix = if ($FrameworkDependent) { '-framework-dependent' } else { '' }
$publishVersion = if ($Version) { $Version } else { (Get-Content -LiteralPath (Join-Path $projectRoot 'VERSION') -Raw).Trim() }
if ($publishVersion -notmatch '\A(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z' -or
    @($publishVersion.Split('.') | Where-Object { $_.Length -gt 5 -or [int]$_ -gt 65535 }).Count -gt 0) {
    throw 'Version must be X.Y.Z, with each component between 0 and 65535.'
}
$publishDirectory = Join-Path $projectRoot ("dist\Krill.Desktop-$publishVersion-win-x64" + $flavorSuffix)
$selfContained = if ($FrameworkDependent) { 'false' } else { 'true' }
if (Test-Path -LiteralPath $publishDirectory) { Remove-Item -LiteralPath $publishDirectory -Recurse -Force }
dotnet publish $projectFile -c Release -r win-x64 --self-contained $selfContained -p:PublishSingleFile=false "-p:Version=$publishVersion" -o $publishDirectory
if ($LASTEXITCODE -ne 0) { throw 'Publish failed.' }
# Version-independent package identity lets already-installed 0.2.0 clients
# validate this native .NET package and return to the WPF application.
$packageIdentity = @{ version = $publishVersion; platform = 'win-x64'; executable = 'Krill.Desktop.exe' } | ConvertTo-Json
[System.IO.File]::WriteAllText((Join-Path $publishDirectory 'krill-package.json'), $packageIdentity, [System.Text.UTF8Encoding]::new($false))
$assemblyVersion = [System.Reflection.AssemblyName]::GetAssemblyName((Join-Path $publishDirectory 'Krill.Desktop.dll')).Version.ToString(3)
if ($assemblyVersion -ne $publishVersion) { throw 'Published assembly version does not match release.' }
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination (Join-Path $publishDirectory 'README.md')
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs') -Destination $publishDirectory -Recurse -Force
$archive = Join-Path $projectRoot ("dist\Krill.Desktop-$publishVersion-win-x64" + $flavorSuffix + '.zip')
Compress-Archive -Path (Join-Path $publishDirectory '*') -DestinationPath $archive -Force
Write-Output $archive
