@echo off
setlocal EnableDelayedExpansion

echo ============================================================
echo  Deploy web-react to GitHub Pages
echo ============================================================
echo.

cd /d "C:\Users\isich\XFChess\web-react"

REM Build the app
echo [1/3] Building web-react...
call npm run build
if %ERRORLEVEL% neq 0 (
    echo ERROR: Build failed
    exit /b 1
)

REM Create temp directory for deployment
set "TEMP_DIR=%TEMP%\gh-pages-deploy-%RANDOM%"
mkdir "%TEMP_DIR%"

echo [2/3] Preparing deployment in %TEMP_DIR%...
cd /d "%TEMP_DIR%"

REM Initialize git and setup gh-pages branch
git init
git remote add origin https://github.com/trilltino/XFChess.git
git checkout -b gh-pages

REM Copy built files
xcopy /E /I /Y "C:\Users\isich\XFChess\web-react\dist\*" .\ >nul

REM Add a .nojekyll file to prevent Jekyll processing
echo. > .nojekyll

echo [3/3] Deploying to GitHub Pages...
git add -A
git commit -m "Deploy web-react to GitHub Pages" --allow-empty
git push origin gh-pages --force

cd /d "C:\Users\isich\XFChess"
rmdir /S /Q "%TEMP_DIR%"

echo.
echo ============================================================
echo  Deployed successfully!
echo ============================================================
echo.
echo Website: https://trilltino.github.io/XFChess
echo.

pause
