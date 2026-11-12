$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $projectRoot 'apps/windows/src/Krill.Desktop/Services/install-update.ps1')

function Assert-Equal($Expected, $Actual) {
    if ($Expected -ne $Actual) { throw "Expected '$Expected', got '$Actual'." }
}

# Check quoting used by Windows PowerShell's Start-Process before replacing the launcher below.
function Start-Process($FilePath, $WorkingDirectory, $ArgumentList) { $script:quotedArguments = @($ArgumentList) }
Start-Krill ([pscustomobject]@{ Destination = [System.IO.Path]::GetTempPath(); Arguments = @('', 'space and quote"', 'C:\some dir\') })
Assert-Equal '""' $script:quotedArguments[0]
Assert-Equal '"space and quote\""' $script:quotedArguments[1]
Assert-Equal '"C:\some dir\\"' $script:quotedArguments[2]
Remove-Item Function:Start-Process

# Exercise the real file replacement and rollback functions in an isolated directory.
$root = Join-Path ([System.IO.Path]::GetTempPath()) ("Krill update '" + [guid]::NewGuid().ToString('N'))
try {
    foreach ($failLaunch in @($false, $true)) {
        $caseRoot = Join-Path $root ([string]$failLaunch)
        $destination = Join-Path $caseRoot 'app'
        $staging = Join-Path $caseRoot 'staging'
        $payload = Join-Path $staging 'payload'
        New-Item -ItemType Directory -Path $destination, $payload -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $destination 'Krill.Desktop.exe') -Value 'old'
        Set-Content -LiteralPath (Join-Path $destination 'personal.txt') -Value 'keep'
        Set-Content -LiteralPath (Join-Path $payload 'Krill.Desktop.exe') -Value 'new'
        Set-Content -LiteralPath (Join-Path $payload 'new-library.dll') -Value 'new library'
        $config = Join-Path $staging 'update.json'
        @{ ProcessId = 999999; Destination = $destination; Payload = $payload; Arguments = @('--demo', "space and 'quote") } |
            ConvertTo-Json | Set-Content -LiteralPath $config -Encoding UTF8
        $script:failLaunch = $failLaunch
        $script:launchCount = 0
        $script:notified = $false
        function Start-Krill($Config) {
            $script:launchCount++
            Assert-Equal "space and 'quote" $Config.Arguments[1]
            if ($script:failLaunch -and (Get-Content -LiteralPath (Join-Path $Config.Destination 'Krill.Desktop.exe')) -eq 'new') {
                throw 'Synthetic launch failure'
            }
        }
        function Show-UpdateFailure { $script:notified = $true }
        Assert-Equal (-not $failLaunch) (Install-KrillUpdate $config)
        Assert-Equal 'keep' (Get-Content -LiteralPath (Join-Path $destination 'personal.txt'))
        Assert-Equal $failLaunch $script:notified
        if ($failLaunch) {
            Assert-Equal 'old' (Get-Content -LiteralPath (Join-Path $destination 'Krill.Desktop.exe'))
            Assert-Equal $false (Test-Path -LiteralPath (Join-Path $destination 'new-library.dll'))
            Assert-Equal 2 $script:launchCount
        } else {
            Assert-Equal 'new' (Get-Content -LiteralPath (Join-Path $destination 'Krill.Desktop.exe'))
            Assert-Equal $false (Test-Path -LiteralPath $staging)
            Assert-Equal 1 $script:launchCount
        }
    }
    Write-Output 'PASS Windows update replacement, argument preservation, unrelated files, and rollback.'
} finally { if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force } }
