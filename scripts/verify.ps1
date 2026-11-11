$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    cargo fmt --all --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting failed.' }
    cargo test --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & ./scripts/windows/test-update.ps1
    cargo build --locked --package krill-desktop --features native-qa
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' }
    $data = Join-Path $env:TEMP 'krill-native-demo'
    $process = Start-Process -FilePath ./target/debug/KrillMac.exe -ArgumentList @('--smoke-test', '--data-dir', ('"{0}"' -f $data), '--output', 'artifacts/rust-native') -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw 'Native smoke verification failed.' }
    $result = Get-Content artifacts/rust-native/verification.json -Raw | ConvertFrom-Json
    if (-not $result.success) { throw 'Native smoke verification did not succeed.' }
} finally { Pop-Location }
