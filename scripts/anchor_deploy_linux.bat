@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set IMAGE=xfchess-anchor-builder
set SOLANA_CONF=%USERPROFILE%\.config\solana

where docker >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Docker is required on PATH.
    exit /b 1
)

if not exist "%SOLANA_CONF%" (
    echo ERROR: Solana config directory not found at %SOLANA_CONF%.
    echo Ensure your Solana wallet and config are in the default location.
    exit /b 1
)

echo Building Anchor 1.1.2 toolchain image...
docker build -f "%ROOT%\docker\anchor-builder.Dockerfile" -t %IMAGE% "%ROOT%"
if %ERRORLEVEL% neq 0 exit /b 1

echo Running anchor build and deploy inside Docker...
docker run --rm -v "%ROOT%:/workspace" -v "%SOLANA_CONF%:/root/.config/solana:ro" -w /workspace %IMAGE% bash -lc "anchor build && anchor deploy"
if %ERRORLEVEL% neq 0 exit /b 1

echo Anchor deploy complete. If a wallet passphrase or keyfile prompt appears, use your standard Solana credentials.
endlocal
