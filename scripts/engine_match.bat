@echo off
REM Engine match: nimzovich-uci vs Rustic via cutechess-cli (SPRT).
REM
REM Prerequisites (one-time):
REM   1. cutechess-cli:  https://github.com/cutechess/cutechess/releases
REM      -> set CUTECHESS to the cutechess-cli.exe path (or put it on PATH)
REM   2. Rustic release binary: https://codeberg.org/mvanthoor/rustic/releases
REM      (Alpha 1 = 1675 CCRL, Alpha 2 = 1815, Alpha 3 = 1865)
REM      -> set RUSTIC to the rustic .exe path
REM   3. Opening book (recommended): 8moves_v3.pgn from the cutechess repo or
REM      https://github.com/official-stockfish/books -> set BOOK to the .pgn path
REM
REM Usage:
REM   scripts\engine_match.bat            (200 games, 10s+0.1s)
REM   set ROUNDS=500 & scripts\engine_match.bat
REM
REM Results land in engine_match.pgn; cutechess prints a running Elo estimate.

setlocal

if "%CUTECHESS%"=="" set CUTECHESS=cutechess-cli.exe
if "%RUSTIC%"==""    set RUSTIC=rustic.exe
if "%ROUNDS%"==""    set ROUNDS=100
if "%TC%"==""        set TC=10+0.1

REM Build the adapter first
cargo build --release -p nimzovich-uci || exit /b 1

set NIMZO=%~dp0..\target\release\nimzovich-uci.exe

set BOOKARGS=
if not "%BOOK%"=="" set BOOKARGS=-openings file=%BOOK% format=pgn order=random

"%CUTECHESS%" ^
  -engine name=Nimzovich cmd="%NIMZO%" proto=uci ^
  -engine name=Rustic    cmd="%RUSTIC%" proto=uci ^
  -each tc=%TC% option.Hash=64 timemargin=200 ^
  -rounds %ROUNDS% -games 2 -repeat ^
  -concurrency 2 ^
  %BOOKARGS% ^
  -recover ^
  -draw movenumber=80 movecount=8 score=10 ^
  -resign movecount=5 score=600 ^
  -sprt elo0=0 elo1=20 alpha=0.05 beta=0.05 ^
  -pgnout engine_match.pgn

endlocal
