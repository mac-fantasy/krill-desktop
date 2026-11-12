param([string]$HarPath, [switch]$LivePublic)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
& (Join-Path $PSScriptRoot 'test-update.ps1')
dotnet build (Join-Path $projectRoot 'Krill.Desktop.slnx') -c Release --nologo
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
$testArguments = @('run', '--project', (Join-Path $projectRoot 'apps\windows\tests\Krill.Core.Tests'), '-c', 'Release', '--no-build', '--')
if ($HarPath) { $testArguments += @('--har', (Resolve-Path -LiteralPath $HarPath).Path) }
if ($LivePublic) { $testArguments += '--live-public' }
& dotnet @testArguments
if ($LASTEXITCODE -ne 0) { throw 'Core verification failed.' }
$outputDirectory = Join-Path $projectRoot 'artifacts\ui'
$smokeDirectory = Join-Path $projectRoot 'artifacts\verify-data'
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$verificationPath = Join-Path $outputDirectory 'verification.json'
$failurePath = Join-Path $outputDirectory 'failure.txt'
if (Test-Path -LiteralPath $verificationPath) { Remove-Item -LiteralPath $verificationPath }
if (Test-Path -LiteralPath $failurePath) { Remove-Item -LiteralPath $failurePath }
$application = Join-Path $projectRoot 'apps\windows\src\Krill.Desktop\bin\Release\net10.0-windows\Krill.Desktop.exe'
$smokeArguments = @('--smoke-test', '--data-dir', ('"{0}"' -f $smokeDirectory), '--output', ('"{0}"' -f $outputDirectory))
$smokeProcess = Start-Process -FilePath $application -ArgumentList $smokeArguments -WindowStyle Hidden -PassThru -Wait
if ($smokeProcess.ExitCode -ne 0) { throw "WPF verification failed. See $failurePath" }
$verification = Get-Content -LiteralPath $verificationPath -Raw | ConvertFrom-Json
if (-not $verification.success) { throw 'WPF verification did not finish successfully.' }
Write-Output ("PASS WPF verification: {0} checks. Images: {1}" -f $verification.checks.Count, $outputDirectory)
