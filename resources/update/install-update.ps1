param([string]$Configuration)

function Start-Krill($Config) {
    # Windows command-line quoting, including trailing backslashes before a quote.
    $quoted = @($config.Arguments | ForEach-Object {
        '"' + ([regex]::Replace([regex]::Replace([string]$_, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1')) + '"'
    })
    $start = @{ FilePath = (Join-Path $config.Destination 'Krill.Desktop.exe'); WorkingDirectory = $config.Destination }
    if ($quoted.Count -gt 0) { $start.ArgumentList = $quoted }
    Start-Process @start | Out-Null
}


function Show-UpdateFailure {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show('更新失败，已尝试恢复原版本。请重试；诊断信息位于临时目录 krill-update.log。', 'Krill Desktop') | Out-Null
}

function Install-KrillUpdate([string]$Configuration) {
    $ErrorActionPreference = 'Stop'
    $config = Get-Content -LiteralPath $Configuration -Raw -Encoding UTF8 | ConvertFrom-Json
    $staging = Split-Path -Parent $Configuration
    $backup = Join-Path $staging 'backup'
    $journal = [System.Collections.Generic.List[object]]::new()
    $logFile = Join-Path ([System.IO.Path]::GetTempPath()) 'krill-update.log'

    try {
        $owner = Get-Process -Id $config.ProcessId -ErrorAction SilentlyContinue
        if ($owner -and -not $owner.WaitForExit(60000)) { throw 'Application did not exit; update cancelled.' }
        # Record only files in the release. Keep unrelated files in portable installations intact.
        foreach ($file in Get-ChildItem -LiteralPath $config.Payload -File -Recurse) {
            $relative = $file.FullName.Substring($config.Payload.Length).TrimStart('\', '/')
            $target = Join-Path $config.Destination $relative
            $saved = Join-Path $backup $relative
            $existed = Test-Path -LiteralPath $target
            if ($existed) {
                New-Item -ItemType Directory -Path (Split-Path -Parent $saved) -Force | Out-Null
                Copy-Item -LiteralPath $target -Destination $saved -Force
            }
            $journal.Add(@{ Target = $target; Saved = $saved; Existed = $existed })
            New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
            # Another demo/account instance may still hold a DLL; retry without killing it.
            for ($attempt = 0; ; $attempt++) {
                try { Copy-Item -LiteralPath $file.FullName -Destination $target -Force; break }
                catch { if ($attempt -ge 30) { throw }; Start-Sleep -Milliseconds 500 }
            }
        }
        Start-Krill $config
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
        return $true
    } catch {
        $_ | Out-String | Add-Content -LiteralPath $logFile
        for ($index = $journal.Count - 1; $index -ge 0; $index--) {
            $entry = $journal[$index]
            try {
                if ($entry.Existed) { Copy-Item -LiteralPath $entry.Saved -Destination $entry.Target -Force }
                else { Remove-Item -LiteralPath $entry.Target -Force -ErrorAction SilentlyContinue }
            } catch { $_ | Out-String | Add-Content -LiteralPath $logFile }
        }
        if (-not $owner -or $owner.HasExited) {
            try { Start-Krill $config } catch { $_ | Out-String | Add-Content -LiteralPath $logFile }
        }
        Show-UpdateFailure
        return $false
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    if (-not $Configuration) { throw 'Configuration is required.' }
    if (-not (Install-KrillUpdate $Configuration)) { exit 1 }
}
