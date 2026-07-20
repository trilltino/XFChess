@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set STAGE=%ROOT%\release\linux
set IMAGE=xfchess-linux
set CONTAINER=xfchess-linux-extract

where docker >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Docker is required on PATH.
    exit /b 1
)

docker build -f "%ROOT%\docker\game-linux.Dockerfile" -t %IMAGE% "%ROOT%"
if %ERRORLEVEL% neq 0 exit /b 1

docker rm -f %CONTAINER% >nul 2>&1
docker create --name %CONTAINER% %IMAGE% >nul
if %ERRORLEVEL% neq 0 exit /b 1

if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE%"

docker cp %CONTAINER%:/app/xfchess "%STAGE%\xfchess"
if %ERRORLEVEL% neq 0 (
    docker rm -f %CONTAINER% >nul 2>&1
    exit /b 1
)
docker cp %CONTAINER%:/app/assets "%STAGE%\assets"
docker rm -f %CONTAINER% >nul 2>&1

echo Linux release staged at %STAGE%
echo Copy the folder to a Linux host and run: chmod +x xfchess ^&^& ./xfchess
endlocal
