# PowerShell script to run XFChess with increased stack size
# Set minimum stack size to 1GB for all threads to prevent stack overflow
# in Bevy's internal task pools and chess engine deep search
# Increased from 8MB -> 32MB -> 128MB -> 256MB -> 512MB -> 1GB due to extremely deep AI recursive search

# Clear debug log from previous run
if (Test-Path "debug.log") {
    Remove-Item "debug.log" -Force
    Write-Host "Cleared previous debug.log" -ForegroundColor Yellow
}

# Kill any existing XFChess processes
Get-Process -Name "XFChess" -ErrorAction SilentlyContinue | Stop-Process -Force

$env:RUST_MIN_STACK = "1073741824"
$env:RUST_BACKTRACE = "1"
Write-Host "Starting XFChess with 1GB stack size (RUST_MIN_STACK=$env:RUST_MIN_STACK)..." -ForegroundColor Green
Write-Host "Debug output will be saved to debug.log" -ForegroundColor Cyan

# Run and tee output to both console and debug.log
cargo run --release 2>&1 | Tee-Object -FilePath "debug.log"
