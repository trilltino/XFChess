$ErrorActionPreference = 'SilentlyContinue'

$root = (Get-Location).Path
Write-Host "[CLEANUP] Stopping all stale XFChess dev processes..." -ForegroundColor Cyan

# backend pid file
$pidFile = Join-Path $root 'backend/.backend.pid'
if (Test-Path $pidFile) {
    $oldPid = Get-Content $pidFile -ErrorAction SilentlyContinue
    if ($oldPid) {
        Stop-Process -Id $oldPid -Force -ErrorAction SilentlyContinue
        Write-Host "  Killed backend PID $oldPid"
    }
    Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
}

# port-level cleanup
foreach ($p in 5173, 5174, 5175, 5176, 5177, 8090, 8091, 7454, 7455, 7456, 7457, 7458, 7459, 7460, 7461, 7462, 7463, 7464) {
    $conns = Get-NetTCPConnection -State Listen -LocalPort $p -ErrorAction SilentlyContinue
    foreach ($conn in $conns) {
        if ($conn.OwningProcess) {
            Stop-Process -Id $conn.OwningProcess -Force -ErrorAction SilentlyContinue
            Write-Host "  Killed port-$p owner PID $($conn.OwningProcess)"
        }
    }
}

# repo-owned dev processes
foreach ($proc in (Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)) {
    if ($proc.ProcessId -eq $PID) { continue }
    $cmd = [string]$proc.CommandLine
    $name = [string]$proc.Name
    if ($cmd -match 'kill_stale_xfchess_dev\.ps1') { continue }
    if (($name -match 'signing-server|xfchess|xfchess-tauri|node|npm|npx|vite|powershell') -and
        ($cmd -match 'XFChess|xfchessdotcom|wallet-ui|tournament-admin|tmp\\dev|tmp\\dev2|signing-server.exe|xfchess.exe|xfchess-tauri.exe|npx vite|vite --port|npm run dev|npm run build')) {
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
        Write-Host "  Killed repo-owned process PID $($proc.ProcessId) ($($proc.Name))"
    }
}

foreach ($n in 'signing-server','xfchess','xfchess-tauri','xfchess-viz') {
    Stop-Process -Name $n -Force -ErrorAction SilentlyContinue
}

if (Test-Path (Join-Path $root 'scripts/kill_wallet_popups.ps1')) {
    Write-Host '  Closing stale wallet-popup (XFChess #) windows...' -ForegroundColor Yellow
    powershell -NoProfile -File (Join-Path $root 'scripts/kill_wallet_popups.ps1')
}

foreach ($pattern in @(
    (Join-Path $env:TEMP 'xfchess-wallet-*.port'),
    (Join-Path $root 'tmp/dev*.ps1'),
    (Join-Path $root 'tmp/dev2*.ps1')
)) {
    if (Test-Path $pattern) {
        Remove-Item $pattern -Force -ErrorAction SilentlyContinue
    }
}

Start-Sleep -Milliseconds 250
Write-Host '[CLEANUP] Done' -ForegroundColor Green
