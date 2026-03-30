@echo off
REM ─────────────────────────────────────────────────────────────────────────────
REM admin_tournament.bat — Thin wrapper around admin_tournament.exe
REM
REM Usage (from project root):
REM   scripts\admin_tournament.bat create  --name "Weekly Cup" --entry-fee 0.05
REM   scripts\admin_tournament.bat start   --id <id>
REM   scripts\admin_tournament.bat record  --id <id> --match-index <0|1|2> --winner <pubkey>
REM   scripts\admin_tournament.bat advance --id <id>
REM   scripts\admin_tournament.bat status  --id <id>
REM ─────────────────────────────────────────────────────────────────────────────
setlocal
cd /d "%~dp0.."

set EXE=target\debug\admin_tournament.exe

if not exist %EXE% (
    echo [BUILD] Building admin_tournament...
    cargo build --features solana --bin admin_tournament
    if errorlevel 1 ( echo [ERROR] Build failed. & pause & exit /b 1 )
)

%EXE% %*
endlocal
