@echo off
:: Builds xfchess_game.so with size-optimized flags.
:: Uses profile.release from Cargo.toml with opt-level=z for minimal size.
::
:: Output: target\deploy\xfchess_game.so (at workspace root)

echo [build_program] Building with size-optimized profile (opt-level=z, lto=true)...

cargo build-sbf --manifest-path programs\xfchess-game\Cargo.toml
if errorlevel 1 (
    echo [build_program] FAILED
    exit /b 1
)

echo.
echo [build_program] Done.
for %%F in (target\deploy\xfchess_game.so) do echo Binary size: %%~zF bytes
