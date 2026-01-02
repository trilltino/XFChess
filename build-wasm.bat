@echo off
REM Build script for compiling XFChess to WebAssembly (Windows)

echo Building XFChess for WebAssembly...

REM Check if wasm32 target is installed
rustup target list --installed | findstr "wasm32-unknown-unknown" >nul
if errorlevel 1 (
    echo Installing wasm32-unknown-unknown target...
    rustup target add wasm32-unknown-unknown
)

REM Check if wasm-bindgen-cli is installed
where wasm-bindgen >nul 2>&1
if errorlevel 1 (
    echo Installing wasm-bindgen-cli...
    cargo install wasm-bindgen-cli
)

REM Set RUSTFLAGS for WASM
set RUSTFLAGS=--cfg getrandom_backend="wasm_js"

REM Build the web package for WASM
echo Building xfchess-web for wasm32-unknown-unknown...
cargo build --package xfchess-web --target wasm32-unknown-unknown --release
if errorlevel 1 (
    echo Build failed!
    exit /b 1
)

REM Generate JavaScript bindings
echo Generating JavaScript bindings...
wasm-bindgen ^
    --out-dir web\pkg ^
    --out-name xfchess_web ^
    --target web ^
    target\wasm32-unknown-unknown\release\xfchess_web.wasm
if errorlevel 1 (
    echo wasm-bindgen failed!
    exit /b 1
)

echo.
echo Build complete! Output files are in web\pkg\
echo To serve the application, use:
echo   cd web ^&^& trunk serve
echo or
echo   cd web ^&^& python -m http.server

