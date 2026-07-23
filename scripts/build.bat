@echo off
setlocal EnableDelayedExpansion

echo ========================================
echo XFChess Full Build Script
echo ========================================
echo This script compiles all components of XFChess
echo.
echo Components to build:
echo   - Backend (signing-server-http)
echo   - Game (xfchess with Solana)
echo   - Tauri host (xfchess-tauri)
echo   - Wallet UI (tauri/wallet-ui)
echo   - Web frontend (xfchessdotcom)
echo   - All Rust crates
echo.
echo ========================================
echo.

set SCRIPT_DIR=%~dp0
for %%i in ("%SCRIPT_DIR%..") do set "ROOT=%%~fi"
set START_TIME=%TIME%

:: Check for required tools
echo [PRE-CHECK] Verifying build dependencies...
where cargo >nul 2>&1
if errorlevel 1 (
    echo ERROR: Rust/Cargo not found. Install Rust from https://rustup.rs
    pause
    exit /b 1
)

where node >nul 2>&1
if errorlevel 1 (
    echo ERROR: Node.js not found. Install Node.js from https://nodejs.org
    pause
    exit /b 1
)

where npm >nul 2>&1
if errorlevel 1 (
    echo ERROR: npm not found. Install Node.js from https://nodejs.org
    pause
    exit /b 1
)

echo All dependencies found.
echo.

:: Build all Rust crates first (dependencies)
echo [1/7] Building all Rust crates (optimized)...
cd /d "%ROOT%"
cargo build --release
if errorlevel 1 (
    echo ERROR: Rust crate build failed
    pause
    exit /b 1
)
echo  Rust crates built successfully
echo.

:: Build Backend
echo [2/7] Building XFChess Signing Server (HTTP)...
cd /d "%ROOT%\backend"
cargo build --bin signing-server-http --release
if errorlevel 1 (
    echo ERROR: Backend build failed
    pause
    exit /b 1
)
echo  Backend built successfully
echo.

:: Build Game
echo [3/7] Building XFChess Game (Solana Enabled) - LTO + native CPU + Bevy optimized...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana --release
if errorlevel 1 (
    echo ERROR: Game build failed
    pause
    exit /b 1
)
echo  Game built successfully
echo.

:: Build Wallet UI
echo [4/7] Building Wallet UI (tauri/wallet-ui)...
cd /d "%ROOT%\tauri\wallet-ui"
if exist "dist" rmdir /s /q "dist"
call npm install
if errorlevel 1 (
    echo ERROR: Wallet UI npm install failed
    pause
    exit /b 1
)
call npm run build
if errorlevel 1 (
    echo ERROR: Wallet UI build failed
    pause
    exit /b 1
)
echo  Wallet UI built successfully
echo.

:: Build Tauri host
echo [5/7] Building XFChess Tauri host - LTO + native CPU...
cd /d "%ROOT%"
cargo build -p xfchess-tauri --release
if errorlevel 1 (
    echo ERROR: Tauri build failed
    pause
    exit /b 1
)
echo  Tauri host built successfully
echo.

:: Build Web frontend
echo [6/7] Building Web Frontend (xfchessdotcom)...
cd /d "%ROOT%\xfchessdotcom"
call npm install
if errorlevel 1 (
    echo ERROR: Web frontend npm install failed
    pause
    exit /b 1
)
call npm run build
if errorlevel 1 (
    echo ERROR: Web frontend build failed
    pause
    exit /b 1
)
echo  Web frontend built successfully
echo.

:: Build Solana programs (if they exist)
echo [7/7] Building Solana Programs...
cd /d "%ROOT%\programs"
if exist "build" (
    cd /d "%ROOT%\programs"
    anchor build
    if errorlevel 1 (
        echo WARNING: Solana program build failed (may not be critical)
        echo Continuing...
    ) else (
        echo  Solana programs built successfully
    )
) else (
    echo ℹ No programs directory found, skipping Solana program build
)
echo.

:: Summary
echo ========================================
echo BUILD COMPLETE
echo ========================================
echo.
echo Build artifacts location:
echo   Backend:        %ROOT%\target\release\signing-server-http.exe
echo   Game:           %ROOT%\target\release\xfchess.exe
echo   Tauri:          %ROOT%\target\release\xfchess-tauri.exe
echo   Wallet UI:      %ROOT%\tauri\wallet-ui\dist\
echo   Web Frontend:   %ROOT%\xfchessdotcom\dist\
echo.
echo Build started: %START_TIME%
echo Build finished: %TIME%
echo.
echo To run the full stack, use:
echo   scripts\run_offline.bat
echo.
echo ========================================
endlocal

