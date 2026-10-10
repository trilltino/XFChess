# Docker-based Anchor toolchain. Build by default; -Deploy also deploys.
param([switch]$Deploy)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$image = 'xfchess-anchor-builder'
$solanaConfig = Join-Path $env:USERPROFILE '.config/solana'

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { throw 'Docker is required on PATH.' }
if ($Deploy -and -not (Test-Path $solanaConfig -PathType Container)) {
    throw "Solana config directory not found: $solanaConfig"
}

docker build -f "$root/docker/anchor-builder.Dockerfile" -t $image $root
if ($LASTEXITCODE -ne 0) { throw 'Anchor toolchain image build failed.' }

$dockerArgs = @('run', '--rm', '-v', "${root}:/workspace", '-w', '/workspace')
if ($Deploy) {
    $dockerArgs += @('-v', "${solanaConfig}:/root/.config/solana:ro")
}
$dockerArgs += $image
if ($Deploy) {
    $dockerArgs += @('bash', '-lc', 'anchor build && anchor deploy')
} else {
    $dockerArgs += @('anchor', 'build')
}
docker @dockerArgs
if ($LASTEXITCODE -ne 0) { throw 'Anchor command failed.' }
