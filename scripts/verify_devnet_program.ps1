#!/usr/bin/env pwsh
# Compare deployed code with the local binary, allowing trailing zero padding
# from a larger prior ProgramData allocation. Usage: just verify-program [-Build]

param(
    [string]$ProgramId = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU",
    [string]$RpcUrl    = "https://api.devnet.solana.com",
    [string]$LocalSo   = "target/deploy/xfchess_game.so",
    [switch]$Build
)

if ($Build) {
    Write-Host "Building locally (cargo build-sbf)..."
    cargo build-sbf --manifest-path programs/xfchess-game/Cargo.toml
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo build-sbf failed - aborting comparison."
        exit 1
    }
}

if (-not (Test-Path $LocalSo)) {
    Write-Error "$LocalSo not found. Run with -Build, or run just build-program first."
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
