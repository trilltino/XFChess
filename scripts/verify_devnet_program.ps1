#!/usr/bin/env pwsh
# Read-only check: does the program actually deployed on devnet match the
# locally-built .so? Answers "is devnet stale" without deploying anything -
# a mismatch is the trigger for a separate, deliberately-approved
# "anchor deploy", not something this script does itself.
#
# Compares only the first min(local, remote) bytes, not full-file hashes.
# BPFLoaderUpgradeable ProgramData accounts never shrink on upgrade - if the
# current binary is smaller than the account's historical high-water mark,
# `solana program dump` still returns the full allocated account length,
# padded with trailing zero bytes. A naive whole-file hash compare flags
# that padding as a mismatch even when the actual deployed code is exact -
# confirmed by hand on 2026-07-31 (a real upgrade whose first N bytes hashed
# identically to the local .so still showed a "MISMATCH" against the old,
# whole-file version of this script because of a 2,816-byte zero tail).
#
# Usage:
#   .\scripts\verify_devnet_program.ps1
#   .\scripts\verify_devnet_program.ps1 -Build            # rebuild locally first
#   .\scripts\verify_devnet_program.ps1 -RpcUrl https://api.devnet.solana.com

param(
    [string]$ProgramId = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU",
    [string]$RpcUrl    = "https://api.devnet.solana.com",
    [string]$LocalSo   = "target/deploy/xfchess_game.so",
    [switch]$Build
)

if ($Build) {
    Write-Host "Building locally (anchor build)..."
    anchor build
    if ($LASTEXITCODE -ne 0) {
        Write-Error "anchor build failed - aborting comparison."
        exit 1
    }
}

if (-not (Test-Path $LocalSo)) {
    Write-Error "$LocalSo not found. Run with -Build, or run anchor build first."
    exit 1
}

$localBytes = [System.IO.File]::ReadAllBytes($LocalSo)
Write-Host "Local  ($LocalSo): $($localBytes.Length) bytes"

$remoteFile = New-TemporaryFile
try {
    Write-Host "Dumping deployed program $ProgramId from $RpcUrl..."
    solana program dump $ProgramId $remoteFile.FullName --url $RpcUrl | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "solana program dump failed - is the program actually deployed at $ProgramId on $RpcUrl?"
        exit 1
    }

    $remoteBytes = [System.IO.File]::ReadAllBytes($remoteFile.FullName)
    Write-Host "Devnet ($ProgramId): $($remoteBytes.Length) bytes"

    if ($remoteBytes.Length -lt $localBytes.Length) {
        Write-Warning "MISMATCH - deployed account ($($remoteBytes.Length) bytes) is smaller than the local build ($($localBytes.Length) bytes). Devnet cannot be running this build; a redeploy is needed."
        exit 1
    }

    $sha = [System.Security.Cryptography.SHA256]::Create()
    $localHash = [BitConverter]::ToString($sha.ComputeHash($localBytes)) -replace '-', ''
    $comparableRemote = $remoteBytes[0..($localBytes.Length - 1)]
    $remoteHash = [BitConverter]::ToString($sha.ComputeHash($comparableRemote)) -replace '-', ''

    $trailing = if ($remoteBytes.Length -gt $localBytes.Length) { $remoteBytes[$localBytes.Length..($remoteBytes.Length - 1)] } else { @() }
    $trailingIsZero = ($trailing.Length -eq 0) -or ((($trailing | Measure-Object -Sum).Sum) -eq 0)

    Write-Host "Local hash (full):               $localHash"
    Write-Host "Devnet hash (first $($localBytes.Length) bytes): $remoteHash"
    if ($trailing.Length -gt 0) {
        Write-Host "Devnet trailing $($trailing.Length) bytes all zero: $trailingIsZero (expected padding from a prior larger deploy - not a mismatch on its own)"
    }

    if ($localHash -eq $remoteHash -and $trailingIsZero) {
        Write-Host "MATCH - devnet is running exactly this source tree's build." -ForegroundColor Green
        exit 0
    } elseif ($localHash -eq $remoteHash) {
        Write-Warning "Code matches, but devnet has $($trailing.Length) non-zero trailing bytes beyond the current build's length - unexpected, investigate before trusting this as a clean match."
        exit 1
    } else {
        Write-Warning "MISMATCH - devnet does NOT match the local build. A redeploy (anchor deploy / anchor upgrade) may be needed before trusting devnet test results against current source."
        exit 1
    }
} finally {
    Remove-Item -Path $remoteFile.FullName -Force -ErrorAction SilentlyContinue
}
