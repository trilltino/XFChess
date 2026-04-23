@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set STAGE=%ROOT%\release\hetzner-backend
set APP_STAGE=%STAGE%\opt\xfchess
set WEB_STAGE=%APP_STAGE%\web
set DATA_STAGE=%APP_STAGE%\data
set BACKUP_STAGE=%APP_STAGE%\backups
set SYSTEMD_STAGE=%STAGE%\etc\systemd\system
set NGINX_STAGE=%STAGE%\etc\nginx\sites-available
set SERVER=178.104.55.19

where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo cargo not found
    exit /b 1
)
where npm >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo npm not found
    exit /b 1
)

pushd "%ROOT%\backend"
echo Backend binary will be built on the Hetzner Linux host by deploy\deploy.ps1
if %ERRORLEVEL% neq 0 exit /b 1
popd

pushd "%ROOT%\web-solana"
if not exist ".env.production" (
    echo VITE_BACKEND_URL=http://%SERVER%:8090> .env.production
)
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%APP_STAGE%"
mkdir "%WEB_STAGE%"
mkdir "%DATA_STAGE%"
mkdir "%BACKUP_STAGE%"
mkdir "%SYSTEMD_STAGE%"
mkdir "%NGINX_STAGE%"
mkdir "%APP_STAGE%\keys"
mkdir "%STAGE%\deploy"

copy /Y "%ROOT%\deploy\xfchess-backend.service" "%SYSTEMD_STAGE%\xfchess-backend.service" >nul
copy /Y "%ROOT%\deploy\nginx.conf" "%NGINX_STAGE%\xfchess" >nul
copy /Y "%ROOT%\deploy\.env.example" "%APP_STAGE%\.env.example" >nul
if exist "%ROOT%\deploy\.env.production" copy /Y "%ROOT%\deploy\.env.production" "%APP_STAGE%\.env.production" >nul
copy /Y "%ROOT%\deploy\deploy.ps1" "%STAGE%\deploy\deploy.ps1" >nul
copy /Y "%ROOT%\deploy\rollback.ps1" "%STAGE%\deploy\rollback.ps1" >nul
xcopy /E /I /Y "%ROOT%\web-solana\dist" "%WEB_STAGE%" >nul

(
echo @echo off
echo setlocal
echo echo Hetzner backend package staged at %%~dp0..
echo echo.
echo echo Files intended for server layout:
echo echo   /opt/xfchess/signing-server-http  ^(built on server from Git commit^)
echo echo   /opt/xfchess/web
echo echo   /opt/xfchess/.env
echo echo   /etc/systemd/system/xfchess-backend.service
echo echo   /etc/nginx/sites-available/xfchess
echo echo.
echo echo Recommended next step:
echo echo   powershell -ExecutionPolicy Bypass -File deploy\deploy.ps1 -Server %SERVER% -User root
echo endlocal
) > "%STAGE%\print-next-step.bat"

echo Hetzner backend package staged at %STAGE%
endlocal
