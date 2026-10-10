# Stage a Windows folder for local testing or a remote backend.
param(
    [ValidateSet('local', 'dev')][string]$Target = 'local',
    [string]$BackendUrl = 'https://xfchess.com',
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $root "release/$Target"
$profile = if ($Debug) { 'debug' } else { 'release' }
$buildFlags = if ($Debug) { @() } else { @('--release') }
if ($Target -eq 'local') { $BackendUrl = 'http://127.0.0.1:8090' }
$backendUri = [uri]$BackendUrl
if (-not $backendUri.IsAbsoluteUri -or $backendUri.Scheme -notin @('http', 'https')) {
    throw 'BackendUrl must be an absolute HTTP(S) URL.'
}
if ($BackendUrl -match '["%\r\n&|<>^]') { throw 'BackendUrl contains unsupported launcher characters.' }

Push-Location $root
try {
    # Only the wallet UI is embedded in this package; web releases use CI.
    Push-Location 'tauri/wallet-ui'
    try {
        npm ci
        if ($LASTEXITCODE -ne 0) { throw 'Wallet UI dependency install failed.' }
        npm run build
        if ($LASTEXITCODE -ne 0) { throw 'Wallet UI build failed.' }
    } finally { Pop-Location }

    if ($Target -eq 'local') {
        cargo build -p backend --bin signing-server @buildFlags
        if ($LASTEXITCODE -ne 0) { throw 'Backend build failed.' }
    }
    cargo build --bin xfchess --features solana @buildFlags
    if ($LASTEXITCODE -ne 0) { throw 'Game build failed.' }
    cargo build -p xfchess-tauri @buildFlags
    if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }

    # Resolve and check the staging directory before removing a prior package.
    $stage = [IO.Path]::GetFullPath($stage)
    $releaseRoot = [IO.Path]::GetFullPath((Join-Path $root 'release'))
    if ((Split-Path $stage -Parent) -ne $releaseRoot) { throw "Invalid staging path: $stage" }
    if (Test-Path $stage) {
        if ((Get-Item -LiteralPath $stage).Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Staging directory must not be a link: $stage"
        }
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    Copy-Item -LiteralPath "$root/assets" -Destination "$stage/assets" -Recurse
    foreach ($binary in @('xfchess', 'xfchess-tauri')) {
        Copy-Item -LiteralPath "$root/target/$profile/$binary.exe" -Destination $stage
    }
    if (Test-Path "$root/stockfish.exe") {
        Copy-Item -LiteralPath "$root/stockfish.exe" -Destination $stage
    }

    $backendLaunch = ''
    if ($Target -eq 'local') {
        Copy-Item -LiteralPath "$root/target/$profile/signing-server.exe" -Destination $stage
        # Preserve local backend configuration and fill missing development secrets.
        $lines = if (Test-Path "$root/backend/.env") { @(Get-Content "$root/backend/.env") } else { @() }
        foreach ($key in @('JWT_SECRET', 'IDENTITY_ENCRYPTION_KEY', 'IDENTITY_SALT')) {
            if (-not ($lines | Where-Object { $_ -match "^$key=.+" })) {
                $lines = @($lines | Where-Object { $_ -notmatch "^$key=" })
                $bytes = New-Object byte[] 32
                $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
                try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
                $lines += "$key=$([BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant())"
            }
        }
        $lines = @($lines | Where-Object { $_ -notmatch '^(SIGNING_SERVICE_URL|SIGNING_PORT|SESSION_DB_URL|VAULT_DB_URL)=' })
        $lines += @("SIGNING_SERVICE_URL=$BackendUrl", 'SIGNING_PORT=8090',
                    'SESSION_DB_URL=sqlite://sessions.db?mode=rwc', 'VAULT_DB_URL=sqlite://vault.db?mode=rwc')
        [IO.File]::WriteAllLines("$stage/.env", [string[]]$lines)
        $backendLaunch = 'start "XFChess Backend" /D "%~dp0" "%~dp0signing-server.exe"' + "`r`ntimeout /t 2 /nobreak >nul"
    }
    $launcher = @"
@echo off
setlocal
set "BACKEND_URL=$BackendUrl"
set "SIGNING_SERVICE_URL=$BackendUrl"
set "XFCHESS_BACKEND_PORT=8090"
set "SIGNING_SERVER_PORT=8090"
$backendLaunch
start "XFChess" /D "%~dp0" "%~dp0xfchess-tauri.exe"
endlocal
"@
    [IO.File]::WriteAllText("$stage/launch.bat", $launcher, [Text.Encoding]::ASCII)
    Write-Host "Package staged at $stage"
} finally { Pop-Location }
