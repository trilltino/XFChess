param([string]$PackageName = 'com.trilltino.xfchess')
$ErrorActionPreference = 'Stop'

# Install the debug APK first. run-as executes with the same app sandbox as the game.
$packageDetails = & adb shell dumpsys package $PackageName
if ($LASTEXITCODE -ne 0) { throw 'Connect a physical ARM64 device and install the debug APK first.' }
$libraryLine = $packageDetails | Select-String 'nativeLibraryDir=' | Select-Object -First 1
if (-not $libraryLine) { throw "No nativeLibraryDir found for $PackageName" }
$nativeDirectory = ($libraryLine.Line -split 'nativeLibraryDir=', 2)[1].Trim()
$binaryPath = "$nativeDirectory/libstockfish.so"

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = 'adb'
$startInfo.Arguments = "shell run-as $PackageName $binaryPath"
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardInput = $true
$startInfo.RedirectStandardOutput = $true
$process = [System.Diagnostics.Process]::Start($startInfo)

function Wait-UciLine([string]$Pattern) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        $readTask = $process.StandardOutput.ReadLineAsync()
        $remaining = [Math]::Max(1, [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds)
        if (-not $readTask.Wait($remaining)) { throw "Stockfish timed out waiting for $Pattern" }
        $line = $readTask.Result
        if ($null -eq $line) { throw "Stockfish exited before $Pattern" }
        if ($line -match $Pattern) { return $line }
    }
    throw "Stockfish timed out waiting for $Pattern"
}

try {
    $process.StandardInput.WriteLine('uci')
    $process.StandardInput.Flush()
    Wait-UciLine '^uciok$'
    $process.StandardInput.WriteLine('isready')
    $process.StandardInput.Flush()
    Wait-UciLine '^readyok$'
    $process.StandardInput.WriteLine('position startpos')
    $process.StandardInput.WriteLine('go movetime 100')
    $process.StandardInput.Flush()
    Wait-UciLine '^bestmove [a-h][1-8][a-h][1-8][qrbn]?( |$)'
    $process.StandardInput.WriteLine('quit')
    $process.StandardInput.Flush()
    if (-not $process.WaitForExit(5000)) { throw 'Stockfish did not exit after quit' }
    if ($process.ExitCode -ne 0) { throw "Android Stockfish exited with $($process.ExitCode)" }
    Write-Output 'Android Stockfish handshake and search passed.'
} finally {
    if (-not $process.HasExited) { $process.Kill() }
    $process.Dispose()
}
