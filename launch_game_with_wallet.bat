@echo off
chcp 65001 >nul
setlocal EnableDelayedExpansion

:: ╔═══════════════════════════════════════════════════════════╗
:: ║     XF Chess - Native Game Launcher with Wallet           ║
:: ╚═══════════════════════════════════════════════════════════╝
::
:: This script launches the native Bevy game with wallet session data.
::
:: Usage:
::   launch_game_with_wallet.bat [session_json_file]
::   launch_game_with_wallet.bat [session_json_string]
::
:: The session data contains:
::   - walletPubkey: The player's wallet public key
::   - sessionSigner: The ephemeral session keypair public key
::   - sessionSignerSecret: The session keypair secret key (array)
::   - expiresAt: Session expiry timestamp
::   - gameId: Optional game ID to join
::   - role: 'host' or 'joiner'

set SESSION_DATA=%~1

if "%~1"=="" (
    echo ╔═══════════════════════════════════════════════════════════╗
    echo ║  XF Chess - Native Game Launcher with Wallet Session     ║
    echo ╚═══════════════════════════════════════════════════════════╝
    echo.
    echo Usage:
    echo   %~nx0 ^<session_json_file^>
    echo   %~nx0 ^<session_json_string^>
    echo.
    echo Example:
    echo   %~nx0 xfchess_session.json
    echo.
    pause
    exit /b 1
)

echo ╔═══════════════════════════════════════════════════════════╗
echo ║       Launching XF Chess with Wallet Session             ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.

:: Check if argument is a file
if exist "%~1" (
    echo Reading session from file: %~1
    for /f "usebackq delims=" %%a in ("%~1") do (
        set SESSION_DATA=%%a
    )
) else (
    echo Using session data from command line
)

:: Extract wallet pubkey for display
for /f "tokens=2 delims=:" %%a in ('echo !SESSION_DATA! ^| findstr "walletPubkey"') do (
    set WALLET_PUBKEY=%%a
    set WALLET_PUBKEY=!WALLET_PUBKEY:"=!
    set WALLET_PUBKEY=!WALLET_PUBKEY:,=!
    set WALLET_PUBKEY=!WALLET_PUBKEY: =!
)

:: Extract role
for /f "tokens=2 delims=:" %%a in ('echo !SESSION_DATA! ^| findstr "role"') do (
    set ROLE=%%a
    set ROLE=!ROLE:"=!
    set ROLE=!ROLE:,=!
    set ROLE=!ROLE: =!
)

:: Extract gameId if present
for /f "tokens=2 delims=:" %%a in ('echo !SESSION_DATA! ^| findstr "gameId"') do (
    set GAME_ID=%%a
    set GAME_ID=!GAME_ID:"=!
    set GAME_ID=!GAME_ID:,=!
    set GAME_ID=!GAME_ID: =!
)

:: Display session info
echo Session Details:
echo   Wallet: !WALLET_PUBKEY:~0,20!...
echo   Role: !ROLE!
if defined GAME_ID (
    if not "!GAME_ID!"=="undefined" (
        echo   Game ID: !GAME_ID:~0,20!...
    )
)
echo.

:: Save session to temp file for the game to read
set SESSION_FILE=%TEMP%\xfchess_session_%RANDOM%.json
echo !SESSION_DATA! > "!SESSION_FILE!"
echo Session saved to: !SESSION_FILE!
echo.

:: Set environment variables for the game
set XFCHESS_SESSION_FILE=!SESSION_FILE!
set XFCHESS_SESSION_DATA=!SESSION_DATA!

:: Find game executable
set GAME_EXE=%~dp0target\release\xfchess.exe
if not exist "!GAME_EXE!" (
    set GAME_EXE=%~dp0target\debug\xfchess.exe
)

if not exist "!GAME_EXE!" (
    echo ERROR: Game executable not found
    echo Looking for: target\release\xfchess.exe or target\debug\xfchess.exe
    echo.
    echo Please build the game first:
    echo   cargo build --release
    echo.
    pause
    exit /b 1
)

echo Game executable: !GAME_EXE!
echo.

:: Launch the game with session file path
:: The game will read the session from the environment variable
start "XF Chess - !ROLE!" "!GAME_EXE!"

echo.
echo Game launched successfully!
echo.
echo The game will read the session from: !SESSION_FILE!
echo Session file will be cleaned up when you close this window.
echo.

echo Press any key to close this window...
echo (Game will continue running)
pause >nul

:: Cleanup session file
del "!SESSION_FILE!" 2>nul
