@echo off
setlocal
set SCRIPT_DIR=%~dp0
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
set XFCHESS_BACKEND_PORT=8090
set SIGNING_SERVER_PORT=8090
if exist "%SCRIPT_DIR%signing-server-http.exe" start "XFChess Signing Server" /D "%SCRIPT_DIR%" "%SCRIPT_DIR%signing-server-http.exe"
timeout /t 2 /nobreak >nul
start "XFChess" /D "%SCRIPT_DIR%" "%SCRIPT_DIR%xfchess-tauri.exe"
endlocal
