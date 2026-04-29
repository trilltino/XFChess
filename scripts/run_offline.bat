@echo off
setlocal EnableDelayedExpansion

echo XFChess Local Fullstack Launcher
echo -------------------------------
echo This script launches the XFChess game with the local signing backend and Solana features.
echo It excludes the web-solana frontend and runs entirely on your local machine.
echo.

set SCRIPT_DIR=%~dp0
set ROOT=%SCRIPT_DIR%..
set TARGET_DIR=%ROOT%\target\debug

:: --- Environment Configuration (Synced with backend/.env) ---
set XFCHESS_FAST_LOCAL_BUILD=1
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090

:: Secrets from your .env
set JWT_SECRET=137a895ebd9506dad79ba1f6c7d1119ad1446f7214710d93a0743f72deb5b5f3
set IDENTITY_ENCRYPTION_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
set IDENTITY_SALT=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789

:: Solana / Authority Keys
set SOLANA_RPC_URL=https://api.devnet.solana.com
set PROGRAM_ID=A5HtSnmyTPohayj9633D9queFFmL2ep6u45nv1v4Wj3W
set FEE_PAYER_KEYS=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set VPS_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set KYC_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set HOST_TREASURY_PUBKEY=uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C

:: --- Build Backend ---
echo Building XFChess Signing Server (HTTP)...
cd /d "%ROOT%\backend"
cargo build --bin signing-server-http
if errorlevel 1 (
    echo Backend build failed.
    pause
    exit /b 1
)

:: --- Build Game ---
echo Building XFChess Game (Solana Enabled) from latest source...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana
if errorlevel 1 (
    echo Game build failed.
    pause
    exit /b 1
)

:: --- Build Wallet UI (must come before Tauri so the binary embeds the fresh dist) ---
echo Building Wallet UI from latest source...
cd /d "%ROOT%\tauri\wallet-ui"
if exist "dist" rmdir /s /q "dist"
call npm install
call npm run build
if errorlevel 1 (
    echo Wallet UI build failed.
    pause
    exit /b 1
)

echo Building XFChess Tauri host from latest source...
cd /d "%ROOT%"
cargo build -p xfchess-tauri
if errorlevel 1 (
    echo Tauri build failed.
    pause
    exit /b 1
)

:: --- Launch Services ---
echo.
echo [1/3] Launching Signing Server on http://127.0.0.1:8090...
set ALLOWED_ORIGINS=http://localhost:7454,http://localhost:5173
set RUST_LOG=info
start "XFChess Backend" /D "%ROOT%\backend" cmd /c "%TARGET_DIR%\signing-server-http.exe 2>&1 || pause"

:: Wait for backend to start
timeout /t 3 /nobreak >nul

echo [2/3] Launching XFChess (Tauri hosts wallet UI + HTTP server on port 7454)...
set RELEASE_DIR=%ROOT%\target\debug
if exist "%ROOT%\stockfish.exe" (
    set STOCKFISH_PATH=%ROOT%\stockfish.exe
) else if exist "%ROOT%\references\Stockfish\stockfish.exe" (
    set STOCKFISH_PATH=%ROOT%\references\Stockfish\stockfish.exe
) else (
    set STOCKFISH_PATH=%RELEASE_DIR%\stockfish.exe
)

set XFCHESS_WALLET_MODE=tauri
start "XFChess Tauri" /D "%ROOT%" /MIN cmd /c "%RELEASE_DIR%\xfchess-tauri.exe || pause"

:: Wait for Tauri TCP bridge to initialize before launching game
timeout /t 2 /nobreak >nul

echo [3/4] Launching XFChess Game...
start "XFChess Game" /D "%ROOT%" cmd /c "%RELEASE_DIR%\xfchess.exe || pause"

echo [4/4] Launching web-solana frontend on http://localhost:5173...
start "XFChess Web" /D "%ROOT%\web-solana" cmd /c "npm run dev"

:: Wait a moment then open the browser
timeout /t 4 /nobreak >nul
start http://localhost:5173

echo.
echo ========================================
echo XFChess Local Environment
echo ========================================
echo Backend:        http://127.0.0.1:8090
echo Tauri/Wallet:   http://127.0.0.1:7454 (managed by xfchess-tauri.exe)
echo Web Frontend:   http://localhost:5173
echo Program ID:     %PROGRAM_ID%
echo Solana RPC:     %SOLANA_RPC_URL%
echo.
echo Game launched. When you click "Connect Wallet", it will open
echo the Tauri wallet popup window for wallet signing.
echo.
echo Close the game, wallet server, backend, and web server when finished.
echo ========================================
echo.
endlocal
