param([switch]$FrameworkDependent, [string]$Version)
# Backward-compatible Windows entry point.
& (Join-Path $PSScriptRoot 'windows\publish.ps1') @PSBoundParameters
