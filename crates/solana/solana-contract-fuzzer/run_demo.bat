@echo off
echo === Solana Contract Fuzzer Demo ===
echo.
echo This demonstrates the fuzzer output format.
echo For actual devnet testing, you need a funded keypair.
echo.
echo To run on devnet with real transactions:
echo.
echo 1. Create a Solana keypair with devnet SOL:
echo    solana-keygen new --outfile ~/.config/solana/id.json
echo    solana airdrop 2 ~/.config/solana/id.json --url devnet
echo.
echo 2. Run the fuzzer:
echo    cargo run -p solana-contract-fuzzer --release -- -i 100 -o fuzzer_results.txt
echo.
echo 3. Or with custom options:
echo    cargo run -p solana-contract-fuzzer --release -- ^
echo        --iterations 1000 ^
echo        --accounts 5 ^
echo        --min-sol 0.1 ^
echo        --output results.txt ^
echo        --verbose
echo.
echo === Sample Output Format ===
echo.
type fuzzer_results.txt 2>nul || echo (No results file yet - run the fuzzer first)
echo.
pause
