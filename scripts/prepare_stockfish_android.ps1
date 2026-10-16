$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$workspaceDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$binaryPath = Join-Path $workspaceDirectory 'mobile/app/src/main/jniLibs/arm64-v8a/libstockfish.so'
if (Test-Path -LiteralPath $binaryPath -PathType Leaf) { return }

$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$stagingDirectory = Join-Path $temporaryRoot ('xfchess-stockfish-android-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
try {
    $archivePath = Join-Path $stagingDirectory 'stockfish.tar'
    $curlCommand = Get-Command curl.exe,curl -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $curlCommand) { throw 'curl is required to download Stockfish' }
    & $curlCommand.Source -fL --retry 3 --retry-delay 5 --connect-timeout 10 --max-time 120 --silent --show-error -o $archivePath 'https://github.com/official-stockfish/Stockfish/releases/download/sf_16/stockfish-android-armv8.tar'
    if ($LASTEXITCODE -ne 0) { throw 'Cannot download Stockfish Android archive' }
    & tar -xf $archivePath -C $stagingDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Cannot extract Stockfish Android archive' }
    $binaries = @(Get-ChildItem -LiteralPath $stagingDirectory -File -Recurse -Filter 'stockfish-android-armv8')
    if ($binaries.Count -ne 1) { throw 'Expected exactly one Stockfish ARMv8 executable' }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $binaryPath) | Out-Null
    Copy-Item -LiteralPath $binaries[0].FullName -Destination $binaryPath
    Write-Output "Prepared Stockfish 16: $binaryPath"
} finally {
    $resolvedStagingDirectory = [IO.Path]::GetFullPath($stagingDirectory)
    if (-not $resolvedStagingDirectory.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $resolvedStagingDirectory) -notlike 'xfchess-stockfish-android-*') {
        throw 'Refusing to remove a directory outside the Stockfish staging location'
    }
    Remove-Item -LiteralPath $resolvedStagingDirectory -Recurse -Force
}
