@echo off
REM Build script for compiling XFChess to WebAssembly and preparing for Leptos webpage
REM This script builds the WASM version and sets up everything needed to run in the browser

setlocal enabledelayedexpansion

echo ========================================
echo XFChess WebAssembly Build Script
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
echo [1/5] Checking wasm32-unknown-unknown target...
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

REM Step 2: Check and install wasm-bindgen-cli
echo [2/5] Checking wasm-bindgen-cli...
where wasm-bindgen >nul 2>&1
if errorlevel 1 (
    echo Installing wasm-bindgen-cli...
    cargo install wasm-bindgen-cli
    if errorlevel 1 (
        echo ERROR: Failed to install wasm-bindgen-cli!
        pause
        exit /b 1
    )
    echo wasm-bindgen-cli installed successfully.
) else (
    echo wasm-bindgen-cli already installed.
)
echo.

REM Step 3: Check and install Trunk (for serving Leptos app)
echo [3/5] Checking Trunk...
where trunk >nul 2>&1
if errorlevel 1 (
    echo "Installing Trunk (required for Leptos web app)..."
    cargo install trunk
    if errorlevel 1 (
        echo WARNING: Failed to install Trunk. You can still build manually.
        echo You may need to install it separately: cargo install trunk
    ) else (
        echo Trunk installed successfully.
    )
) else (
    echo Trunk already installed.
)
echo.

REM Step 4: Build the web package for WASM
echo [4/5] Building xfchess-web for wasm32-unknown-unknown...
echo This may take several minutes...
echo.

REM Set RUSTFLAGS for WASM
set RUSTFLAGS=--cfg getrandom_backend=^"wasm_js^"

REM Clean previous build (optional, comment out if you want incremental builds)
REM echo Cleaning previous build...
REM cargo clean --package xfchess-web --target wasm32-unknown-unknown

REM Build in release mode for smaller size
cargo build --package xfchess-web --target wasm32-unknown-unknown --release
if errorlevel 1 (
    echo.
    echo ERROR: Build failed!
    echo Check the error messages above for details.
    pause
    exit /b 1
)
echo Build successful!
echo.

REM Step 5: Generate JavaScript bindings
echo [5/5] Generating JavaScript bindings with wasm-bindgen...

REM Create output directory if it doesn't exist
if not exist "web\pkg" mkdir web\pkg

wasm-bindgen ^
    --out-dir web\pkg ^
    --out-name xfchess_web ^
    --target web ^
    target\wasm32-unknown-unknown\release\xfchess_web.wasm

if errorlevel 1 (
    echo.
    echo ERROR: wasm-bindgen failed!
    echo Check the error messages above for details.
    pause
    exit /b 1
)
echo JavaScript bindings generated successfully!
echo.

REM Summary
echo ========================================
echo Build Complete!
echo ========================================
echo.
echo Output files are in: web\pkg\
echo.
echo To serve the application, you have two options:
echo.
echo "Option 1 - Using Trunk (Recommended for Leptos):"
echo   cd web
echo   trunk serve
echo   Then open http://localhost:8080 in your browser
echo.
echo Option 2 - Using Python HTTP server:
echo   cd web
echo   python -m http.server 8000
echo   Then open http://localhost:8000 in your browser
echo.
echo "Option 3 - Using wasm-server-runner (for testing):"
echo   cargo run --package xfchess-web --target wasm32-unknown-unknown
echo.
echo ========================================
echo.

REM Ask if user wants to start the server
set /p START_SERVER="Do you want to start Trunk server and open in browser? (Y/N): "
if /i "!START_SERVER!"=="Y" (
    echo.
    echo Starting Trunk server...
    echo The webpage will open automatically in your browser at http://localhost:8080
    echo Press Ctrl+C to stop the server.
    echo.
    cd web
    trunk serve --open
) else (
    echo.
    echo Build complete! Run 'trunk serve --open' in the web directory when ready.
)

pause

