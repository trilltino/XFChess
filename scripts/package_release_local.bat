@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set STAGE=%ROOT%\release\local
set ASSET_STAGE=%STAGE%\assets
set LOCAL_IP=127.0.0.1
set BACKEND_HTTP_PORT=8090
set SIGNING_PORT=8090
set BACKEND_URL=http://%LOCAL_IP%:%BACKEND_HTTP_PORT%
set SIGNING_SERVICE_URL=http://%LOCAL_IP%:%SIGNING_PORT%

where npm >nul 2>&1
if %ERRORLEVEL% neq 0 exit /b 1
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 exit /b 1
where powershell >nul 2>&1
if %ERRORLEVEL% neq 0 exit /b 1

if not exist "%ROOT%\backend\.env" type nul > "%ROOT%\backend\.env"
findstr /B /C:"JWT_SECRET=" "%ROOT%\backend\.env" >nul 2>&1
if %ERRORLEVEL% neq 0 (
    for /f "tokens=*" %%i in ('powershell -NoProfile -Command "-join ((1..64) ^| ForEach-Object { '{0:x}' -f (Get-Random -Max 16) })"') do set JWT_SECRET=%%i
    >> "%ROOT%\backend\.env" echo JWT_SECRET=%JWT_SECRET%
)
findstr /B /C:"IDENTITY_ENCRYPTION_KEY=" "%ROOT%\backend\.env" >nul 2>&1
if %ERRORLEVEL% neq 0 (
    for /f "tokens=*" %%i in ('powershell -NoProfile -Command "-join ((1..64) ^| ForEach-Object { '{0:x}' -f (Get-Random -Max 16) })"') do set IDENTITY_ENCRYPTION_KEY=%%i
    >> "%ROOT%\backend\.env" echo IDENTITY_ENCRYPTION_KEY=%IDENTITY_ENCRYPTION_KEY%
)
findstr /B /C:"IDENTITY_SALT=" "%ROOT%\backend\.env" >nul 2>&1
if %ERRORLEVEL% neq 0 (
    for /f "tokens=*" %%i in ('powershell -NoProfile -Command "-join ((1..64) ^| ForEach-Object { '{0:x}' -f (Get-Random -Max 16) })"') do set IDENTITY_SALT=%%i
    >> "%ROOT%\backend\.env" echo IDENTITY_SALT=%IDENTITY_SALT%
)

pushd "%ROOT%\web-solana"
set VITE_BACKEND_URL=%BACKEND_URL%
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

pushd "%ROOT%\tauri\wallet-ui"
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

cargo build -p backend --bin signing-server --release
if %ERRORLEVEL% neq 0 exit /b 1

cargo build --bin xfchess --release
if %ERRORLEVEL% neq 0 exit /b 1

cargo build -p xfchess-tauri --release
if %ERRORLEVEL% neq 0 exit /b 1

if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE%"
mkdir "%ASSET_STAGE%"
xcopy /E /I /Y "%ROOT%\assets" "%ASSET_STAGE%" >nul
copy /Y "%ROOT%\target\release\xfchess.exe" "%STAGE%\xfchess.exe" >nul
copy /Y "%ROOT%\target\release\xfchess-tauri.exe" "%STAGE%\xfchess-tauri.exe" >nul
copy /Y "%ROOT%\target\release\signing-server.exe" "%STAGE%\signing-server.exe" >nul
if exist "%ROOT%\stockfish.exe" copy /Y "%ROOT%\stockfish.exe" "%STAGE%\stockfish.exe" >nul
if exist "%ROOT%\backend\.env" copy /Y "%ROOT%\backend\.env" "%STAGE%\.env" >nul

(
echo @echo off
echo setlocal
echo set SCRIPT_DIR=%%~dp0
echo set BACKEND_URL=http://127.0.0.1:8090
echo set SIGNING_SERVICE_URL=http://127.0.0.1:8090
echo set XFCHESS_BACKEND_PORT=8090
echo set SIGNING_SERVER_PORT=8090
echo if exist "%%SCRIPT_DIR%%signing-server.exe" start "XFChess Signing Server" /D "%%SCRIPT_DIR%%" "%%SCRIPT_DIR%%signing-server.exe"
echo timeout /t 2 /nobreak ^>nul
echo start "XFChess" /D "%%SCRIPT_DIR%%" "%%SCRIPT_DIR%%xfchess-tauri.exe"
echo endlocal
) > "%STAGE%\launch_local_release.bat"

echo Local release package staged at %STAGE%
endlocal
