@echo off
setlocal EnableDelayedExpansion

echo.
echo  ============================================================
echo   XFChess  ^|  Tournament Admin Console (VPS CLI)
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Check for ADMIN_API_KEY ---
echo.
echo  [Security Check] Checking for ADMIN_API_KEY...

if not defined ADMIN_API_KEY (
    echo.
    echo  [!] ADMIN_API_KEY not set in environment.
    echo.
    echo  You have two options:
    echo.
    echo  [1] Set it now (will be saved for this session only)
    echo  [2] Exit and set it manually in System Environment Variables
    echo.
    set /p CHOICE="Select option (1-2): "
    
    if "!CHOICE!"=="1" (
        echo.
        echo  Paste your API key (generate one with generate_api_key.bat):
        set /p API_KEY="API Key: "
        set ADMIN_API_KEY=!API_KEY!
        echo  [OK] API key set for this session.
    ) else (
        echo.
        echo  To set ADMIN_API_KEY permanently:
        echo    1. Windows Settings ^> System ^> About ^> Advanced System Settings
        echo    2. Environment Variables ^> New (User variables)
        echo    3. Variable name: ADMIN_API_KEY
        echo    4. Variable value: (your secure key)
        echo.
        echo  To generate a secure key, run: scripts\generate_api_key.bat
        echo.
        pause
        exit /b 1
    )
)

REM --- Check for SIGNING_SERVICE_URL ---
echo.
echo  [Config Check] Checking for SIGNING_SERVICE_URL...

if not defined SIGNING_SERVICE_URL (
    echo  [!] SIGNING_SERVICE_URL not set. Using default ngrok URL.
    echo  [!] Default: https://unrejuvenated-philologically-trudi.ngrok-free.app
    echo.
    echo  [1] Use default (for ngrok with auto-generated URL)
    echo  [2] Enter custom URL (e.g., http://localhost:8090 for local)
    echo  [3] Exit and set manually
    echo.
    set /p URL_CHOICE="Select option (1-3): "
    
    if "!URL_CHOICE!"=="1" (
        set SIGNING_SERVICE_URL=https://unrejuvenated-philologically-trudi.ngrok-free.app
        echo  [OK] Using default ngrok URL.
    ) else if "!URL_CHOICE!"=="2" (
        echo.
        set /p CUSTOM_URL="Enter backend URL (e.g., http://localhost:8090): "
        set SIGNING_SERVICE_URL=!CUSTOM_URL!
        echo  [OK] Using custom URL: !CUSTOM_URL!
    ) else (
        echo.
        echo  Exiting. Please set SIGNING_SERVICE_URL and try again.
        pause
        exit /b 1
    )
) else (
    echo  [OK] Using: %SIGNING_SERVICE_URL%
)

REM --- Display current configuration ---
echo.
echo  ============================================================
echo   Configuration
echo  ============================================================
echo.
echo  Backend URL: %SIGNING_SERVICE_URL%
echo  API Key:     ********%ADMIN_API_KEY:~-4%
echo.

REM --- Build and run vps_admin ---
echo  [Building] Compiling vps_admin in release mode...
cd backend
cargo build --bin vps_admin --release --quiet
if %ERRORLEVEL% neq 0 (
    echo  [ERROR] Build failed. Check for compilation errors.
    pause
    exit /b 1
)
echo  [OK] Build successful.

echo.
echo  ============================================================
echo   Launching Tournament Admin Console
echo  ============================================================
echo.
echo  Menu Options:
echo    1 - List all tournaments
echo    2 - Create new tournament
echo    3 - View tournament details
echo    4 - View tournament bracket/matches
echo    5 - Record match result
echo    6 - Link match to on-chain game ID
echo    7 - Cancel tournament
echo    8 - Calculate/view prizes
echo    0 - Exit
echo.
echo  ============================================================
echo.

REM --- Run the admin console ---
cargo run --bin vps_admin --release --quiet

echo.
echo  [Done] Admin console closed.
echo.

pause
endlocal
