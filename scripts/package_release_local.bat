@echo off
setlocal
cd /d "%~dp0.."
set ROOT=%cd%
set STAGE=%ROOT%\release\local
set ASSET_STAGE=%STAGE%\assets
set TARGET_SUBDIR=release
set BUILD_FLAGS=--release
if /I "%XFCHESS_FAST_LOCAL_BUILD%"=="1" (
    set TARGET_SUBDIR=debug
    set BUILD_FLAGS=
)
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

pushd "%ROOT%\xfchessdotcom"
set VITE_BACKEND_URL=%BACKEND_URL%
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

pushd "%ROOT%\tauri\wallet-ui"
call npm run build
if %ERRORLEVEL% neq 0 exit /b 1
popd

cargo build -p backend --bin signing-server-http %BUILD_FLAGS%
if %ERRORLEVEL% neq 0 exit /b 1

cargo build --bin xfchess %BUILD_FLAGS%
if %ERRORLEVEL% neq 0 exit /b 1

cargo build -p xfchess-tauri %BUILD_FLAGS%
if %ERRORLEVEL% neq 0 exit /b 1

if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE%"
mkdir "%ASSET_STAGE%"
xcopy /E /I /Y "%ROOT%\assets" "%ASSET_STAGE%" >nul
copy /Y "%ROOT%\target\%TARGET_SUBDIR%\xfchess.exe" "%STAGE%\xfchess.exe" >nul
copy /Y "%ROOT%\target\%TARGET_SUBDIR%\xfchess-tauri.exe" "%STAGE%\xfchess-tauri.exe" >nul
copy /Y "%ROOT%\target\%TARGET_SUBDIR%\signing-server-http.exe" "%STAGE%\signing-server-http.exe" >nul
if exist "%ROOT%\stockfish.exe" copy /Y "%ROOT%\stockfish.exe" "%STAGE%\stockfish.exe" >nul
if not exist "%STAGE%\stockfish.exe" (
    echo Stockfish not found - downloading from official repository...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/official-stockfish/Stockfish/releases/download/sf_16/stockfish-windows-x86-64-modern.exe' -OutFile '%STAGE%\stockfish.exe'"
    if %ERRORLEVEL% neq 0 (
        echo Failed to download Stockfish. AI features will not work.
    )
)
if exist "%ROOT%\backend\.env" copy /Y "%ROOT%\backend\.env" "%STAGE%\.env" >nul
if not exist "%STAGE%\.env" type nul > "%STAGE%\.env"
powershell -NoProfile -Command "$path='%STAGE%\.env'; $lines=@(); if (Test-Path $path) { $lines=Get-Content $path }; function Get-Val([string]$name, [string]$fallback) { $matchesForKey = $lines | Where-Object { $_ -match ('^' + [regex]::Escape($name) + '=') }; foreach ($entry in $matchesForKey) { $value = ($entry -replace ('^' + [regex]::Escape($name) + '='), '').Trim(); if (-not [string]::IsNullOrWhiteSpace($value)) { return $value } }; return $fallback }; $jwt=Get-Val 'JWT_SECRET' '%JWT_SECRET%'; $enc=Get-Val 'IDENTITY_ENCRYPTION_KEY' '%IDENTITY_ENCRYPTION_KEY%'; $salt=Get-Val 'IDENTITY_SALT' '%IDENTITY_SALT%'; $forcedKeys=@('JWT_SECRET','IDENTITY_ENCRYPTION_KEY','IDENTITY_SALT','SIGNING_SERVICE_URL','SIGNING_PORT','SESSION_DB_URL','VAULT_DB_URL'); $seen=New-Object System.Collections.Generic.HashSet[string]; $filtered=foreach($line in $lines){ if($line -match '^(?<key>[A-Z0-9_]+)='){ $key=$matches['key']; if($forcedKeys -contains $key){ continue }; if($seen.Contains($key)){ continue }; [void]$seen.Add($key) }; $line }; $out=@($filtered); if(-not [string]::IsNullOrWhiteSpace($jwt)){ $out += ('JWT_SECRET=' + $jwt) }; if(-not [string]::IsNullOrWhiteSpace($enc)){ $out += ('IDENTITY_ENCRYPTION_KEY=' + $enc) }; if(-not [string]::IsNullOrWhiteSpace($salt)){ $out += ('IDENTITY_SALT=' + $salt) }; $out += 'SIGNING_SERVICE_URL=http://127.0.0.1:8090'; $out += 'SIGNING_PORT=8090'; $out += 'SESSION_DB_URL=sqlite://sessions.db?mode=rwc'; $out += 'VAULT_DB_URL=sqlite://vault.db?mode=rwc'; [System.IO.File]::WriteAllLines($path, $out)"

(
echo @echo off
echo setlocal
echo set SCRIPT_DIR=%%~dp0
echo set BACKEND_URL=http://127.0.0.1:8090
echo set SIGNING_SERVICE_URL=http://127.0.0.1:8090
echo set XFCHESS_BACKEND_PORT=8090
echo set SIGNING_SERVER_PORT=8090
echo if exist "%%SCRIPT_DIR%%signing-server-http.exe" start "XFChess Signing Server" /D "%%SCRIPT_DIR%%" "%%SCRIPT_DIR%%signing-server-http.exe"
echo timeout /t 2 /nobreak ^>nul
echo start "XFChess" /D "%%SCRIPT_DIR%%" "%%SCRIPT_DIR%%xfchess-tauri.exe"
echo endlocal
) > "%STAGE%\launch_local_release.bat"

echo Local release package staged at %STAGE%
endlocal
