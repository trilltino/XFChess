@echo off
:: Builds xfchess_game.so with size-optimized flags.
:: CARGO_PROFILE_RELEASE_OPT_LEVEL=z overrides opt-level=3 from the workspace
:: profile just for this build, shrinking the binary by ~25-35% vs speed-mode.
::
:: Output: target\deploy\xfchess_game.so (at workspace root)

set CARGO_PROFILE_RELEASE_OPT_LEVEL=z

echo [build_program] opt-level=z  lto=true  codegen-units=1

cargo build-sbf --manifest-path programs\xfchess-game\Cargo.toml
if errorlevel 1 (
    echo [build_program] FAILED
    exit /b 1
)

echo.
echo [build_program] Done.
for %%F in (target\deploy\xfchess_game.so) do echo Binary size: %%~zF bytes
