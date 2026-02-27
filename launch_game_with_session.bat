@echo off
setlocal EnableDelayedExpansion

:: ╔═══════════════════════════════════════════════════════════════════════════╗
:: ║     XFChess Game Launcher with Session                                    ║
:: ╚═══════════════════════════════════════════════════════════════════════════╝
::
:: Usage: launch_game_with_session.bat <session_json_file>
::
:: This script launches the XFChess native game client with session parameters
:: downloaded from the web UI.
::

:: Check if JSON file provided
if "%~1"=="" (
    echo.
    echo ╔═══════════════════════════════════════════════════════════════════════════╗
    echo ║  ERROR: No session file provided                                          ║
    echo ╠═══════════════════════════════════════════════════════════════════════════╣
    echo ║  Usage: launch_game_with_session.bat ^<session_json_file^>                ║
    echo ║                                                                           ║
    echo ║  Example:                                                                 ║
    echo ║    launch_game_with_session.bat xfchess_session.json                      ║
    echo ╚═══════════════════════════════════════════════════════════════════════════╝
    echo.
    pause
    exit /b 1
)

set SESSION_FILE=%~1
set GAME_DIR=%~dp0

:: Check if session file exists
if not exist "%SESSION_FILE%" (
    echo.
    echo ╔═══════════════════════════════════════════════════════════════════════════╗
    echo ║  ERROR: Session file not found                                            ║
    echo ╠═══════════════════════════════════════════════════════════════════════════╣
    echo ║  File: %SESSION_FILE%                                                     ║
    echo ║                                                                           ║
    echo ║  Make sure you downloaded the session file from the web UI.               ║
    echo ╚═══════════════════════════════════════════════════════════════════════════╝
    echo.
    pause
    exit /b 1
)

:: Check if game binary exists
if not exist "%GAME_DIR%target\release\xfchess.exe" (
    echo.
    echo ╔═══════════════════════════════════════════════════════════════════════════╗
    echo ║  ERROR: xfchess.exe not found                                             ║
    echo ╠═══════════════════════════════════════════════════════════════════════════╣
    echo ║  Expected: %GAME_DIR%target\release\xfchess.exe                           ║
    echo ║                                                                           ║
    echo ║  Please build the game first:                                             ║
    echo ║    cargo build --release                                                  ║
    echo ╚═══════════════════════════════════════════════════════════════════════════╝
    echo.
    pause
    exit /b 1
)

:: Copy session file to game directory with standard name
set SESSION_DEST=%GAME_DIR%session_config.json
echo 📄 Copying session file to game directory...
copy /Y "%SESSION_FILE%" "%SESSION_DEST%" >nul
if errorlevel 1 (
    echo ❌ Failed to copy session file
    pause
    exit /b 1
)

echo.
echo ╔═══════════════════════════════════════════════════════════════════════════╗
echo ║  Launching XFChess                                                        ║
echo ╠═══════════════════════════════════════════════════════════════════════════╣
echo ║  Session: %SESSION_FILE%                                                  ║
echo ║  Game Dir: %GAME_DIR%                                                     ║
echo ╚═══════════════════════════════════════════════════════════════════════════╝
echo.

:: Change to game directory and launch
cd /d "%GAME_DIR%"

:: Launch the game with session config
start "" "target\release\xfchess.exe" --session-config session_config.json

echo ✅ Game client launched!
echo.
echo 📝 The game will read the session config and connect to:
echo    • Solana Devnet
echo    • Your wallet session
echo    • The wager game
echo.
echo 💡 If the game doesn't appear, check your taskbar.
echo.
timeout /t 3 >nul
