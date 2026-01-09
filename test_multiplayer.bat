@echo off
setlocal enabledelayedexpansion
echo ========================================
echo       XFChess Multiplayer Test (SQLite)
echo ========================================
echo.

:: Store current directory
set "PROJECT_DIR=%~dp0"
cd /d "%PROJECT_DIR%"
echo Project Directory: %PROJECT_DIR%

echo [1/1] Launching Windows Terminal with Backend and Clients...
echo.

:: Create helper batch files for Windows Terminal
echo @echo off > "%TEMP%\xfchess_backend.bat"
echo cd /d "%PROJECT_DIR%" >> "%TEMP%\xfchess_backend.bat"
echo echo Starting Backend (SQLite)... >> "%TEMP%\xfchess_backend.bat"
echo cargo run -p backend >> "%TEMP%\xfchess_backend.bat"
echo pause >> "%TEMP%\xfchess_backend.bat"

echo @echo off > "%TEMP%\xfchess_client1.bat"
echo cd /d "%PROJECT_DIR%" >> "%TEMP%\xfchess_client1.bat"
echo echo Starting Host Client... >> "%TEMP%\xfchess_client1.bat"
echo timeout /t 2 >> "%TEMP%\xfchess_client1.bat"
echo cargo run -p xfchess >> "%TEMP%\xfchess_client1.bat"
echo pause >> "%TEMP%\xfchess_client1.bat"

echo @echo off > "%TEMP%\xfchess_client2.bat"
echo cd /d "%PROJECT_DIR%" >> "%TEMP%\xfchess_client2.bat"
echo echo Starting Guest Client... >> "%TEMP%\xfchess_client2.bat"
echo timeout /t 4 >> "%TEMP%\xfchess_client2.bat"
echo cargo run -p xfchess >> "%TEMP%\xfchess_client2.bat"
echo pause >> "%TEMP%\xfchess_client2.bat"

:: Launch Windows Terminal with the helper scripts
wt -w 0 nt --title "XFChess Backend" "%TEMP%\xfchess_backend.bat" ; nt --title "Host Client" "%TEMP%\xfchess_client1.bat" ; nt --title "Guest Client" "%TEMP%\xfchess_client2.bat"

echo.
echo ========================================
echo  All components launched successfully!
echo ========================================
echo.
echo  Note: Backend defaults to "sqlite:xfchess.db"
echo  (Postgres container is NOT used/required)
echo.
pause
