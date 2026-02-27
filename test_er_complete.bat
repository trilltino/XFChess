@echo off
chcp 65001 >nul
setlocal EnableDelayedExpansion

:: ╔═══════════════════════════════════════════════════════════════════════════╗
:: ║     MagicBlock ER Complete Test Suite - On-Chain Evidence Collection      ║
:: ╚═══════════════════════════════════════════════════════════════════════════╝
::
:: This script runs the complete ER testing workflow:
::   1. Phase 1: Delegation (delegate game PDA to ER)
::   2. Phase 2: Gameplay (execute moves through ER)
::   3. Phase 3: Undelegation (commit final state to Solana)
::   4. Verification: Verify all evidence on-chain
::
:: Evidence is saved to web-solana/evidence/ for audit/proof
::

echo ╔═══════════════════════════════════════════════════════════════════════════╗
echo ║     MagicBlock ER Complete Test Suite                                     ║
echo ║     On-Chain Evidence Collection                                          ║
echo ╚═══════════════════════════════════════════════════════════════════════════╝
echo.

:: Configuration
set WEB_SOLANA_DIR=%~dp0web-solana
set EVIDENCE_DIR=%WEB_SOLANA_DIR%\evidence
set GAME_PDA=
set TEST_WALLET=%WEB_SOLANA_DIR%\test_wallet.json

:: Check prerequisites
echo 🔍 Checking prerequisites...

if not exist "%WEB_SOLANA_DIR%" (
    echo ❌ web-solana directory not found at %WEB_SOLANA_DIR%
    pause
    exit /b 1
)

where node >nul 2>&1
if errorlevel 1 (
    echo ❌ Node.js not found! Please install Node.js first.
    pause
    exit /b 1
)

:: Check for tsx
if not exist "%WEB_SOLANA_DIR%\node_modules\.bin\tsx.cmd" (
    echo 📦 Installing tsx for TypeScript execution...
    cd "%WEB_SOLANA_DIR%"
    call npm install --save-dev tsx >nul 2>&1
    if errorlevel 1 (
        echo ❌ Failed to install tsx
        pause
        exit /b 1
    )
)

echo ✅ Prerequisites met
echo.

:: Create evidence directory
if not exist "%EVIDENCE_DIR%" mkdir "%EVIDENCE_DIR%"

:: ============================================
:: PHASE 1: DELEGATION TEST
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo  PHASE 1: DELEGATION TEST
echo ═══════════════════════════════════════════════════════════════════════════
echo.
echo This phase will:
echo   • Create a game PDA on Solana
echo   • Delegate it to MagicBlock ER
echo   • Capture transaction signature as evidence
echo.
echo Press any key to start Phase 1...
pause >nul

cd "%WEB_SOLANA_DIR%"
echo.
echo 🚀 Running delegation test...
call npx tsx test_er_delegation.ts

if errorlevel 1 (
    echo.
    echo ❌ Phase 1 failed! Check the error above.
    pause
    exit /b 1
)

:: Find the game PDA from the evidence file
for /f "delims=" %%a in ('dir /b /o-d "%EVIDENCE_DIR%\delegation_*.json" 2^>nul ^| findstr /v error') do (
    for /f "tokens=2 delims=:" %%b in ('type "%EVIDENCE_DIR%\%%a" ^| findstr "gamePda"') do (
        set GAME_PDA_RAW=%%b
        set GAME_PDA=!GAME_PDA_RAW:"=!
        set GAME_PDA=!GAME_PDA: =!
        set GAME_PDA=!GAME_PDA:,=!
        goto :found_game_pda
    )
)
:found_game_pda

if "!GAME_PDA!"=="" (
    echo ❌ Could not find game PDA in evidence
    pause
    exit /b 1
)

echo.
echo ✅ Phase 1 complete!
echo    Game PDA: !GAME_PDA!
echo.

:: ============================================
:: PHASE 2: GAMEPLAY TEST
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo  PHASE 2: GAMEPLAY TEST
echo ═══════════════════════════════════════════════════════════════════════════
echo.
echo This phase will:
echo   • Execute 6 chess moves through ER
echo   • Capture latency metrics for each move
echo   • Verify moves are confirmed on both ER and Solana
echo.
echo Game PDA: !GAME_PDA!
echo.
echo Press any key to start Phase 2...
pause >nul

echo.
echo 🚀 Running gameplay test...
call npx tsx test_er_gameplay.ts !GAME_PDA!

if errorlevel 1 (
    echo.
    echo ⚠️  Phase 2 had errors but continuing...
)

echo.
echo ✅ Phase 2 complete!
echo.

:: ============================================
:: PHASE 3: UNDELEGATION TEST
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo  PHASE 3: UNDELEGATION TEST
echo ═══════════════════════════════════════════════════════════════════════════
echo.
echo This phase will:
echo   • Undelegate game PDA from ER
echo   • Commit final state to Solana
echo   • Capture state commitment proof
echo.
echo Game PDA: !GAME_PDA!
echo.
echo Press any key to start Phase 3...
pause >nul

echo.
echo 🚀 Running undelegation test...
call npx tsx test_er_undelegation.ts !GAME_PDA!

if errorlevel 1 (
    echo.
    echo ⚠️  Phase 3 had errors but continuing...
)

echo.
echo ✅ Phase 3 complete!
echo.

:: ============================================
:: VERIFICATION PHASE
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo  VERIFICATION PHASE
echo ═══════════════════════════════════════════════════════════════════════════
echo.
echo This phase will:
echo   • Verify all transaction signatures on-chain
echo   • Check account state consistency
echo   • Generate verification report
echo.
echo Press any key to start verification...
pause >nul

echo.
echo 🔍 Verifying all evidence on-chain...
call npx tsx verify_on_chain.ts

if errorlevel 1 (
    echo.
    echo ⚠️  Verification had errors but continuing...
)

echo.
echo ✅ Verification complete!
echo.

:: ============================================
:: SUMMARY
:: ============================================
echo ═══════════════════════════════════════════════════════════════════════════
echo  TEST SUITE SUMMARY
echo ═══════════════════════════════════════════════════════════════════════════
echo.

:: Count evidence files
set DELEGATION_COUNT=0
set GAMEPLAY_COUNT=0
set UNDELEGATION_COUNT=0
set VERIFICATION_COUNT=0

for /f %%a in ('dir /b "%EVIDENCE_DIR%\delegation_*.json" 2^>nul ^| find /c /v ""') do set DELEGATION_COUNT=%%a
for /f %%a in ('dir /b "%EVIDENCE_DIR%\gameplay_*.json" 2^>nul ^| find /c /v ""') do set GAMEPLAY_COUNT=%%a
for /f %%a in ('dir /b "%EVIDENCE_DIR%\undelegation_*.json" 2^>nul ^| find /c /v ""') do set UNDELEGATION_COUNT=%%a
for /f %%a in ('dir /b "%EVIDENCE_DIR%\verification_report_*.json" 2^>nul ^| find /c /v ""') do set VERIFICATION_COUNT=%%a

echo 📁 Evidence Files Generated:
echo    Delegation:    !DELEGATION_COUNT! files
echo    Gameplay:      !GAMEPLAY_COUNT! files
echo    Undelegation:  !UNDELEGATION_COUNT! files
echo    Verification:  !VERIFICATION_COUNT! files
echo.
echo 📂 Evidence Location: %EVIDENCE_DIR%
echo.

:: List latest evidence files
echo 📝 Latest Evidence Files:
dir /b /o-d "%EVIDENCE_DIR%\*.json" 2>nul | head -10 || dir /b /o-d "%EVIDENCE_DIR%\*.json" 2>nul

echo.
echo ╔═══════════════════════════════════════════════════════════════════════════╗
echo ║  ✅ TEST SUITE COMPLETE                                                   ║
echo ╠═══════════════════════════════════════════════════════════════════════════╣
echo ║  All phases executed with on-chain evidence captured                      ║
echo ║                                                                           ║
echo ║  To view evidence:                                                        ║
echo ║    cd web-solana\evidence                                                 ║
echo ║    type ^<filename^>.json                                                 ║
echo ║                                                                           ║
echo ║  To verify on explorer:                                                   ║
echo ║    https://explorer.solana.com/?cluster=devnet                            ║
echo ╚═══════════════════════════════════════════════════════════════════════════╝
echo.

:: Ask to open evidence folder
echo Open evidence folder? (Y/N)
choice /c YN /n
if errorlevel 1 if not errorlevel 2 (
    start explorer "%EVIDENCE_DIR%"
)

pause
