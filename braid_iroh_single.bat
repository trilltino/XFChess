@echo off
cd /d "%~dp0"
echo ==========================================
echo XFChess Braid-Iroh P2P - Single Instance
echo ==========================================
echo Working directory: %CD%
echo.

:: Check for Rust toolchain
cargo --version >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Rust/Cargo not found! Please install Rust: https://rustup.rs/
    pause
    exit /b 1
)

:: Set Bevy optimization environment variables for faster compilation
set RUSTFLAGS=-C target-cpu=native
set CARGO_PROFILE_DEV_OPT_LEVEL=1
set CARGO_PROFILE_DEV_INCREMENTAL=true
set CARGO_PROFILE_DEV_CODEGEN_UNITS=256
set CARGO_PROFILE_DEV_LTO=false

:: Bevy-specific: Enable fast linking on Windows
set BEVY_ASSET_ROOT=./assets

echo.
echo [1/3] Checking braid-iroh dependency...
cargo check -p braid-iroh --quiet
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] braid-iroh crate check failed!
    cargo check -p braid-iroh
    pause
    exit /b %ERRORLEVEL%
)
echo [OK] braid-iroh dependency is valid

echo.
echo [2/3] Building XFChess in Braid-Iroh P2P mode...
echo Features: braid-i (without solana)
echo Profile: dev-optimized
echo.

cargo build --profile dev-optimized --no-default-features --features braid-i 2>&1
echo.

if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Build failed! Checking for specific errors...
    cargo build --profile dev-optimized --no-default-features --features braid-i
    pause
    exit /b %ERRORLEVEL%
)

echo [OK] Build successful!
echo.

:: Verify executable exists
if exist "target\dev-optimized\xfchess.exe" (
    set "EXE_PATH=target\dev-optimized\xfchess.exe"
) else if exist "target\debug\xfchess.exe" (
    set "EXE_PATH=target\debug\xfchess.exe"
) else (
    echo [ERROR] Could not find xfchess.exe
    pause
    exit /b 1
)

echo [OK] Executable found

:: Generate a unique identity file using timestamp and random
set ID_TIMESTAMP=%date:~-4%%date:~-7,2%%date:~-10,2%%time:~0,2%%time:~3,2%%time:~6,2%
set ID_TIMESTAMP=%ID_TIMESTAMP: =0%
set ID_RANDOM=%random%
set "IDENTITY_FILE=xfchess_identity_%ID_TIMESTAMP%_%ID_RANDOM%.key"

echo.
echo ==========================================
echo Launching XFChess P2P Instance
echo ==========================================
echo Identity file: %IDENTITY_FILE%
echo.

:: Create launcher batch that will be executed in a new window
:: The identity is embedded directly into the batch content
(
echo @echo off
echo cd /d "%%~dp0"
echo set XFCHESS_IDENTITY=%IDENTITY_FILE%
echo echo [XFChess P2P] Starting with identity file: %IDENTITY_FILE%
echo echo.
echo "%%~dp0%EXE_PATH%"
echo if %%ERRORLEVEL%% NEQ 0 echo.
echo if %%ERRORLEVEL%% NEQ 0 echo [ERROR] Exit code: %%ERRORLEVEL%%
echo pause
) > launch_xfchess_p2p.bat

:: Launch the game via the wrapper batch in a new window
echo [Starting XFChess P2P]...
start "XFChess P2P" cmd /k "launch_xfchess_p2p.bat"

echo.
echo [OK] XFChess P2P instance launched!
echo.
echo The Node ID will be printed in the game window when the network initializes.
echo Look for: "Braid network initialized with node ID: ..."
echo.
echo To use P2P:
echo   - Select PLAY ^-> P2P CHESS to host or connect to a peer
echo   - To host: Click "Start Hosting" and share your Node ID
echo   - To join: Enter the host's Node ID and click Connect
echo.
echo NOTE: Each launch uses a unique identity file for different Node IDs
echo.

:: Clean up launcher after a delay to ensure it's been read
timeout /t 3 /nobreak >nul 2>&1
del launch_xfchess_p2p.bat 2>nul

pause
