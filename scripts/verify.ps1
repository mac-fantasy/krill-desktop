param([string]$HarPath, [switch]$LivePublic)
# Backward-compatible Windows entry point.
& (Join-Path $PSScriptRoot 'windows\verify.ps1') @PSBoundParameters
