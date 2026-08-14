@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set IMAGE=xfchess-anchor-builder

where docker >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Docker is required on PATH.
    exit /b 1
)

echo Building Anchor 1.1.2 toolchain image...
docker build -f "%ROOT%\docker\anchor-builder.Dockerfile" -t %IMAGE% "%ROOT%"
if %ERRORLEVEL% neq 0 exit /b 1

echo Running anchor build inside the container...
docker run --rm -v "%ROOT%:/workspace" -w /workspace %IMAGE% anchor build
if %ERRORLEVEL% neq 0 exit /b 1

echo Anchor build complete. Artifact should be available at %ROOT%\target\deploy\xfchess_game.so
endlocal
