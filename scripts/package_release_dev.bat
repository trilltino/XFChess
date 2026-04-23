@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set STAGE=%ROOT%\release\dev
set ASSET_STAGE=%STAGE%\assets
set DEV_HOST=178.104.55.19
set BACKEND_URL=http://%DEV_HOST%
set SIGNING_SERVICE_URL=http://%DEV_HOST%:8090

where npm >nul 2>&1
if %ERRORLEVEL% neq 0 exit /b 1
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 exit /b 1

pushd "%ROOT%\web-solana"
set VITE_BACKEND_URL=%BACKEND_URL%
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

pushd "%ROOT%\tauri\wallet-ui"
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

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
if exist "%ROOT%\stockfish.exe" copy /Y "%ROOT%\stockfish.exe" "%STAGE%\stockfish.exe" >nul

(
echo @echo off
echo setlocal
echo set SCRIPT_DIR=%%~dp0
echo set BACKEND_URL=http://178.104.55.19
echo set SIGNING_SERVICE_URL=http://178.104.55.19:8090
echo start "XFChess" /D "%%SCRIPT_DIR%%" "%%SCRIPT_DIR%%xfchess-tauri.exe"
echo endlocal
) > "%STAGE%\launch_dev_release.bat"

echo Dev release package staged at %STAGE%
endlocal
