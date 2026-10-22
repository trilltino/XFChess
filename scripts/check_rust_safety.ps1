param([switch]$PlatformOnly, [switch]$InventoryOnly)

$ErrorActionPreference = 'Stop'

$rustPaths = @(& git ls-files --cached --others --exclude-standard -- '*.rs' | Sort-Object -Unique)
if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate owned Rust source' }
$extractionPattern = '(?:\.\s*(?:unwrap|expect|unwrap_err|expect_err|unwrap_unchecked)\s*\(|::\s*(?:unwrap|expect|unwrap_err|expect_err|unwrap_unchecked|from_base58_string)\b)'
foreach ($rustPath in $rustPaths) {
    # The vendored macro dependency retains upstream source and its own lint policy.
    if ($rustPath.StartsWith("vendor/proc-macro-error2/")) { continue }
    if (-not (Test-Path -LiteralPath $rustPath -PathType Leaf)) { continue }
    $rustSource = [System.IO.File]::ReadAllText((Join-Path $PWD $rustPath))
    $extraction = [regex]::Match($rustSource, $extractionPattern)
    if ($extraction.Success) {
        $rustLine = $rustSource.Substring(0, $extraction.Index).Split("`n").Length
        throw "Panicking extraction or key decoding in ${rustPath}:$rustLine"
    }
}
Write-Output 'Rust source extraction inventory: clean'
if ($InventoryOnly) { return }

$configurations = @(
    @('-p', 'xfchess'),
    @('-p', 'xfchess', '--features', 'solana'),
    @('-p', 'xfchess-tauri', '-p', 'xfchess-viz'),
    @('-p', 'xfchess-tauri', '--features', 'production,wallet,tournament-admin')
)

if (-not $PlatformOnly) {
    $configurations += @(
        @('-p', 'backend'),
        @('-p', 'backend', '--no-default-features', '--features', 'polling'),
        @('-p', 'xfchess-rules', '-p', 'xfchess-stockfish', '-p', 'xfchess-anticheat',
          '-p', 'swiss-pairing', '-p', 'braid-http', '-p', 'braid_chess',
          '-p', 'xfchess-braid-server', '-p', 'iroh-h3', '-p', 'iroh-h3-axum',
          '-p', 'iroh-h3-client', '-p', 'cu_benchmark'),
        @('-p', 'braid-iroh', '--features', 'proxy'),
        @('-p', 'braid-http', '--all-features'),
        @('-p', 'iroh-h3-client', '--features', 'json'),
        @('-p', 'xfchess-game', '-p', 'xfchess-game-sbf')
    )
}

foreach ($configuration in $configurations) {
    Write-Output ("Checking Rust safety: " + ($configuration -join " "))
    $cargoArguments = @('clippy', '--locked', '--all-targets') + $configuration + @(
        '--', '-D', 'warnings', '-D', 'clippy::unwrap_used', '-D', 'clippy::expect_used',
        '-D', 'unsafe_op_in_unsafe_fn', '-D', 'clippy::undocumented_unsafe_blocks'
    )
    # Windows PowerShell treats Cargo's normal stderr progress as error records.
    $cargoErrorPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & cargo @cargoArguments 2>&1 | ForEach-Object { $_.ToString() }
        $cargoExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $cargoErrorPreference
    }
    if ($cargoExitCode -ne 0) {
        throw "Rust safety check failed: $configuration"
    }
}
