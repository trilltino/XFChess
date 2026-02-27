@echo off
chcp 65001 >nul
setlocal EnableDelayedExpansion

:: ╔═══════════════════════════════════════════════════════════════════════════╗
:: ║     Run ALL ER Tests with Evidence Collection                             ║
:: ╚═══════════════════════════════════════════════════════════════════════════╝

echo ╔═══════════════════════════════════════════════════════════════════════════╗
echo ║     Running ALL MagicBlock ER Tests                                       ║
echo ║     Including Wager Tests                                                 ║
echo ╚═══════════════════════════════════════════════════════════════════════════╝
echo.

set WEB_SOLANA_DIR=%~dp0web-solana
set EVIDENCE_DIR=%WEB_SOLANA_DIR%\evidence

:: Check prerequisites
if not exist "%WEB_SOLANA_DIR%" (
    echo ❌ web-solana directory not found
    exit /b 1
)

where node >nul 2>&1
if errorlevel 1 (
    echo ❌ Node.js not found
    exit /b 1
)

:: Install tsx if needed
if not exist "%WEB_SOLANA_DIR%\node_modules\.bin\tsx.cmd" (
    echo 📦 Installing tsx...
    cd "%WEB_SOLANA_DIR%"
    call npm install --save-dev tsx >nul 2>&1
)

:: Create evidence directory
if not exist "%EVIDENCE_DIR%" mkdir "%EVIDENCE_DIR%"

cd "%WEB_SOLANA_DIR%"

echo 🚀 Starting test suite...
echo Evidence will be saved to: %EVIDENCE_DIR%
echo.

:: ============================================
:: TEST 1: DELEGATION
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo TEST 1/5: Delegation Phase
echo ═══════════════════════════════════════════════════════════════════════════
call npx tsx test_er_delegation.ts
if errorlevel 1 (
    echo ❌ Delegation test failed
) else (
    echo ✅ Delegation test passed
)
echo.

:: ============================================
:: TEST 2: WAGER FLOW
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo TEST 2/5: Wager Flow (Complete Game)
echo ═══════════════════════════════════════════════════════════════════════════
echo Using wallets:
echo   - playtest_white.json (Player 1)
echo   - playtest_black.json (Player 2)
echo.
call npx tsx test_er_wager.ts
if errorlevel 1 (
    echo ❌ Wager test failed
) else (
    echo ✅ Wager test passed
)
echo.

:: Find game PDA from the evidence file for subsequent tests
set GAME_PDA=
for /f "delims=" %%a in ('dir /b /o-d "%EVIDENCE_DIR%\delegation_*.json" 2^>nul ^| findstr /v error') do (
    for /f "tokens=2 delims=:" %%b in ('type "%EVIDENCE_DIR%\%%a" ^| findstr "gamePda"') do (
        set GAME_PDA_RAW=%%b
        set GAME_PDA=!GAME_PDA_RAW:"=!
        set GAME_PDA=!GAME_PDA: =!
        set GAME_PDA=!GAME_PDA:,=!
        goto :found_pda
    )
)
:found_pda

if "!GAME_PDA!"=="" (
    echo ⚠️  No game PDA found - skipping gameplay/undelegation tests
    goto :summary
)

echo 📋 Using Game PDA: !GAME_PDA!
echo.

:: ============================================
:: TEST 3: GAMEPLAY
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo TEST 3/5: Gameplay Phase
echo ═══════════════════════════════════════════════════════════════════════════
call npx tsx test_er_gameplay.ts !GAME_PDA!
if errorlevel 1 (
    echo ⚠️  Gameplay test had errors
) else (
    echo ✅ Gameplay test passed
)
echo.

:: ============================================
:: TEST 4: UNDELEGATION
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo TEST 4/5: Undelegation Phase
echo ═══════════════════════════════════════════════════════════════════════════
call npx tsx test_er_undelegation.ts !GAME_PDA!
if errorlevel 1 (
    echo ⚠️  Undelegation test had errors
) else (
    echo ✅ Undelegation test passed
)
echo.

:: ============================================
:: TEST 5: VERIFICATION
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo TEST 5/5: On-Chain Verification
echo ═══════════════════════════════════════════════════════════════════════════
call npx tsx verify_on_chain.ts
if errorlevel 1 (
    echo ⚠️  Verification had errors
) else (
    echo ✅ Verification passed
)
echo.

:summary
:: ============================================
:: SUMMARY
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo SUMMARY
echo ═══════════════════════════════════════════════════════════════════════════
echo.

:: Count evidence files
echo 📁 Evidence Files Generated:
for %%t in (delegation gameplay undelegation wager verification) do (
    set count=0
    for /f %%a in ('dir /b "%EVIDENCE_DIR%\%%t*.json" 2^>nul ^| find /c /v ""') do set count=%%a
    echo    %%t: !count! files
)

echo.
echo 📂 Full evidence location: %EVIDENCE_DIR%
echo.

:: List all evidence files
echo 📝 All Evidence Files:
dir /b "%EVIDENCE_DIR%\*.json" 2>nul

echo.
echo ╔═══════════════════════════════════════════════════════════════════════════╗
echo ║  ✅ ALL TESTS COMPLETE                                                    ║
echo ╚═══════════════════════════════════════════════════════════════════════════╝
echo.

:: Copy evidence to a timestamped folder for this test run
set TEST_RUN_DIR=%~dp0evidence_%%date:~-4,4%%date:~-10,2%%date:~-7,2%_%%time:~0,2%%time:~3,2%%time:~6,2%
set TEST_RUN_DIR=%TEST_RUN_DIR: =0%
mkdir "%TEST_RUN_DIR%" 2>nul
xcopy "%EVIDENCE_DIR%\*.json" "%TEST_RUN_DIR%\" /Y >nul 2>&1
echo 💾 Evidence copied to: %TEST_RUN_DIR%
echo.

pause
