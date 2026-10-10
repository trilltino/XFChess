@echo off
REM SPRT match against Rustic. Set CUTECHESS and RUSTIC executable paths;
REM BOOK optionally supplies PGN openings. ROUNDS and TC override the defaults.
REM Usage: just engine-match; results are written to engine_match.pgn.

setlocal

if "%CUTECHESS%"=="" set CUTECHESS=cutechess-cli.exe
if "%RUSTIC%"==""    set RUSTIC=rustic.exe
if "%ROUNDS%"==""    set ROUNDS=100
if "%TC%"==""        set TC=10+0.1

REM Build the adapter against the sibling nimzovich repository.
cargo build --release --manifest-path %~dp0..\..\nimzovich\nimzovich-uci\Cargo.toml || exit /b 1

set NIMZO=%~dp0..\..\nimzovich\target\release\nimzovich-uci.exe

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
