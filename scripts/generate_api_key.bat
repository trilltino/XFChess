@echo off
setlocal EnableDelayedExpansion

echo.
echo  ============================================================
echo   XFChess  ^|  API Key Generator
echo  ============================================================
echo.
echo  This tool generates a secure random API key for tournament
echo  admin authentication. The key protects sensitive admin API
echo  endpoints like creating tournaments and recording results.
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Generate a secure random key ---
echo  [Generating] Creating secure random API key...

REM Use PowerShell to generate a cryptographically secure random key
for /f "delims=" %%a in ('powershell -Command "[Convert]::ToBase64String((1..32 ^| ForEach-Object { Get-Random -Maximum 256 } ^| ForEach-Object { [byte]$_ }))"') do set API_KEY=%%a

REM Remove any problematic characters and ensure proper length
set API_KEY=%API_KEY:~0,43%
set API_KEY=%API_KEY:+=-%
set API_KEY=%API_KEY:/=_%

echo.
echo  ============================================================
echo   YOUR NEW API KEY
echo  ============================================================
echo.
echo  !API_KEY!
echo.
echo  [IMPORTANT] Save this key securely! You will need it to:
echo    - Run the VPS admin console (start_vps_admin.bat)
echo    - Access protected tournament admin endpoints
echo    - Create and manage tournaments remotely
echo.

REM --- Save to clipboard ---
echo  !API_KEY! | clip
echo  [Copied] Key copied to clipboard!
echo.

REM --- Option to save to .env file ---
echo  [Options]
echo  [1] Save to backend/.env file (for local development)
echo  [2] Show Windows environment variable setup instructions
echo  [3] Exit (key is in clipboard)
echo.
set /p SAVE_CHOICE="Select option (1-3): "

if "!SAVE_CHOICE!"=="1" (
    echo.
    echo  Saving to backend/.env...
    
    REM Check if .env exists and backup
    if exist "%ROOT%\backend\.env" (
        copy /Y "%ROOT%\backend\.env" "%ROOT%\backend\.env.backup" >nul
        echo  [Backup] Existing .env backed up to .env.backup
    )
    
    REM Write the API key to .env
    echo # XFChess Backend Environment Configuration > "%ROOT%\backend\.env"
    echo # Generated on %date% %time% >> "%ROOT%\backend\.env"
    echo. >> "%ROOT%\backend\.env"
    echo # Admin API Key for tournament management >> "%ROOT%\backend\.env"
    echo ADMIN_API_KEY=!API_KEY! >> "%ROOT%\backend\.env"
    echo. >> "%ROOT%\backend\.env"
    echo # Backend URL (set this to your ngrok or local URL) >> "%ROOT%\backend\.env"
    echo SIGNING_SERVICE_URL=https://unrejuvenated-philologically-trudi.ngrok-free.app >> "%ROOT%\backend\.env"
    echo. >> "%ROOT%\backend\.env"
    echo # Database URLs >> "%ROOT%\backend\.env"
    echo DATABASE_URL=sqlite://sessions.db?mode=rwc >> "%ROOT%\backend\.env"
    echo VAULT_DATABASE_URL=sqlite://vault.db?mode=rwc >> "%ROOT%\backend\.env"
    echo. >> "%ROOT%\backend\.env"
    echo # Solana Configuration >> "%ROOT%\backend\.env"
    echo SOLANA_RPC_URL=https://api.devnet.solana.com >> "%ROOT%\backend\.env"
    echo PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX >> "%ROOT%\backend\.env"
    echo. >> "%ROOT%\backend\.env"
    echo  [OK] API key saved to backend/.env
echo  [OK] You can now run: scripts\start_vps_admin.bat
    echo.
) else if "!SAVE_CHOICE!"=="2" (
    echo.
    echo  ============================================================
    echo   Windows Environment Variable Setup
echo  ============================================================
    echo.
    echo  To set ADMIN_API_KEY permanently:
    echo.
    echo  Method 1 - System Settings (Recommended):
echo    1. Press Win + Pause/Break (or right-click This PC ^> Properties)
echo    2. Click "Advanced system settings"
echo    3. Click "Environment Variables"
echo    4. Under "User variables", click "New"
echo    5. Variable name:  ADMIN_API_KEY
echo    6. Variable value: [paste the key from clipboard]
echo    7. Click OK ^> OK ^> OK
echo    8. Restart any open terminals/IDEs
echo.
    echo  Method 2 - Command Line (Current session only):
echo    set ADMIN_API_KEY=!API_KEY!
echo.
    echo  Method 3 - PowerShell (Current session only):
echo    $env:ADMIN_API_KEY = '!API_KEY!'
echo.
    echo  [Note] Command line methods only persist for the current session.
echo  [Note] Use Method 1 for permanent configuration.
    echo.
)

echo.
echo  ============================================================
echo   Security Notes
echo  ============================================================
echo.
echo  - Never share your API key publicly
echo  - Never commit .env files to git
echo  - Rotate keys periodically (generate new ones)
echo  - If key is compromised, generate a new one immediately
echo.
echo  The backend/.env file is already in .gitignore
echo.

pause
endlocal
