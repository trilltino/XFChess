@echo off
setlocal EnableDelayedExpansion

echo XFChess Multi-Instance Local Launcher
echo -----------------------------------
echo This script launches TWO instances of XFChess for P2P testing.
echo Instance 1: Port 7454, User: Player1
echo Instance 2: Port 7464, User: Player2
echo.

set SCRIPT_DIR=%~dp0
set ROOT=%SCRIPT_DIR%..
set RELEASE_DIR=%ROOT%\target\release

:: --- Environment Configuration ---
set XFCHESS_FAST_LOCAL_BUILD=1
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090

:: Secrets from your .env
set JWT_SECRET=137a895ebd9506dad79ba1f6c7d1119ad1446f7214710d93a0743f72deb5b5f3
set IDENTITY_ENCRYPTION_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
set IDENTITY_SALT=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789

:: Solana / Authority Keys
set SOLANA_RPC_URL=https://api.devnet.solana.com
set PROGRAM_ID=C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf
set FEE_PAYER_KEYS=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set VPS_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set KYC_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set HOST_TREASURY_PUBKEY=uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C

:: --- Build Phase ---
echo [BUILD] Building XFChess Signing Server...
cd /d "%ROOT%\backend"
cargo build --bin signing-server-http
if errorlevel 1 exit /b 1

echo [BUILD] Building XFChess Game...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana
if errorlevel 1 exit /b 1

echo [BUILD] Building Wallet UI...
cd /d "%ROOT%\tauri\wallet-ui"
if exist "dist" rmdir /s /q "dist"
call npm install && call npm run build
if errorlevel 1 exit /b 1

echo [BUILD] Building Tauri host - LTO + native CPU optimized...
cd /d "%ROOT%"
cargo build -p xfchess-tauri --release
if errorlevel 1 exit /b 1

:: --- Launch Phase ---
echo.
echo [1/5] Launching Signing Server...
set ALLOWED_ORIGINS=http://localhost:7454,http://localhost:7464,http://localhost:5173
set RUST_LOG=info
set RELEASE_DIR=%ROOT%\target\release
start "XFChess Backend" /D "%ROOT%\backend" cmd /c "%RELEASE_DIR%\signing-server-http.exe 2>&1 || pause"

timeout /t 3 /nobreak >nul

echo [2/5] Launching Instance 1 (Tauri + Game)...
set XFCHESS_WALLET_PORT=7454
set XFCHESS_USERNAME=Player1
set XFCHESS_WALLET_MODE=tauri
start "XFChess Tauri 1" /D "%ROOT%" /MIN cmd /c "set XFCHESS_WALLET_PORT=7454 && %RELEASE_DIR%\xfchess-tauri.exe || pause"
timeout /t 1 /nobreak >nul
start "XFChess Game 1" /D "%ROOT%" cmd /c "set XFCHESS_WALLET_PORT=7454 && set XFCHESS_USERNAME=Player1 && %RELEASE_DIR%\xfchess.exe || pause"

echo [3/6] Launching Instance 2 (Tauri + Game)...
set XFCHESS_WALLET_PORT=7464
set XFCHESS_USERNAME=Player2
set XFCHESS_WALLET_MODE=tauri
start "XFChess Tauri 2" /D "%ROOT%" /MIN cmd /c "set XFCHESS_WALLET_PORT=7464 && %RELEASE_DIR%\xfchess-tauri.exe || pause"
timeout /t 1 /nobreak >nul
start "XFChess Game 2" /D "%ROOT%" cmd /c "set XFCHESS_WALLET_PORT=7464 && set XFCHESS_USERNAME=Player2 && %RELEASE_DIR%\xfchess.exe || pause"

echo [5/6] Launching web-solana frontend...
start "XFChess Web" /D "%ROOT%\web-solana" cmd /c "npm run dev"

echo [5/5] Opening instances...
timeout /t 4 /nobreak >nul
start http://localhost:7454
start http://localhost:7464

echo.
echo ========================================
echo Multi-Instance Ready
echo ========================================
echo Instance 1: http://localhost:7454 (User: Player1)
echo Instance 2: http://localhost:7464 (User: Player2)
echo Grafana:   http://localhost:3000 (admin/admin)
echo Prometheus: http://localhost:9090
echo ========================================
echo.
endlocal
