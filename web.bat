@echo off
REM Build script for compiling XFChess to WebAssembly and preparing for Leptos webpage
REM This script uses Trunk to build and serve the application

setlocal enabledelayedexpansion

echo ========================================
echo       XFChess Web Launcher
echo ========================================
echo.

REM Check if we're in the right directory
if not exist "web\Cargo.toml" (
    echo ERROR: web\Cargo.toml not found!
    echo Please run this script from the project root directory.
    pause
    exit /b 1
)

REM Step 1: Check and install wasm32 target
echo [1/3] Checking wasm32-unknown-unknown target...
rustup target list --installed | findstr "wasm32-unknown-unknown" >nul
if errorlevel 1 (
    echo Installing wasm32-unknown-unknown target...
    rustup target add wasm32-unknown-unknown
    if errorlevel 1 (
        echo ERROR: Failed to install wasm32-unknown-unknown target!
        pause
        exit /b 1
    )
    echo Target installed successfully.
) else (
    echo Target already installed.
)
echo.

REM Step 2: Check and install Trunk
echo [2/3] Checking Trunk...
where trunk >nul 2>&1
if errorlevel 1 (
    echo "Installing Trunk (required for Leptos web app)..."
    cargo install trunk
    if errorlevel 1 (
        echo WARNING: Failed to install Trunk.
        echo You need to install it manually: cargo install trunk
        pause
        exit /b 1
    ) else (
        echo Trunk installed successfully.
    )
) else (
    echo Trunk already installed.
)
echo.

REM Step 3: Serve with Trunk
echo [3/3] Starting Trunk Server...
echo.

REM Enable detailed logging (RUSTFLAGS now set in .cargo/config.toml for cache consistency)
set RUST_LOG=info,wgpu=warn

echo Configuration:
echo - RUST_LOG: %RUST_LOG%
echo - RUSTFLAGS: (set in .cargo/config.toml)
echo.
echo The application will be built and served by Trunk.
echo Opening http://localhost:8080 in your browser...
echo.
echo Press Ctrl+C to stop the server.
echo.

cd web
trunk serve --open

if errorlevel 1 (
    echo.
    echo ERROR: Trunk serve failed!
    pause
    exit /b 1
)

pause
