# Solana Contract Fuzzer Crate Plan

Create a hybrid Rust fuzzing crate (`crates/solana-contract-fuzzer`) that uses proptest/arbitrary to generate random instruction sequences against the xfchess-game program, with an optional JSON control interface for external test runners.

## Overview

This crate will provide property-based fuzzing for the XFChess Solana smart contracts, targeting the `programs/xfchess-game` Anchor program. It combines fast native Rust fuzzing with a braid-fuzz-compatible JSON interface for integration testing.

## Crate Structure

```
crates/solana-contract-fuzzer/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── fuzzer.rs           # Core fuzzing engine
│   ├── strategies/         # proptest Arbitrary implementations
│   │   ├── mod.rs
│   │   ├── instructions.rs # Instruction generation
│   │   ├── state.rs        # Game state mutations
│   │   └── accounts.rs     # Account/keypair generation
│   ├── runner/             # Test execution
│   │   ├── mod.rs
│   │   ├── solana_runner.rs # solana_program_test wrapper
│   │   └── results.rs      # Fuzz results collection
│   ├── controller/           # Optional JSON interface
│   │   ├── mod.rs
│   │   ├── protocol.rs     # Message types
│   │   ├── stdio.rs        # Stdin/stdout bridge
│   │   └── tcp.rs          # TCP server option
│   └── invariants/         # Property checks
│       ├── mod.rs
│       ├── game_rules.rs   # Chess rules invariants
│       └── economic.rs     # Wager/escrow invariants
└── tests/
    ├── fuzz_basic.rs       # Smoke tests
    └── fuzz_regression.rs  # Known bug reproductions
```

## Key Components

### 1. Core Fuzzer (`fuzzer.rs`)
- `FuzzerConfig`: Seed, iteration count, strategy weights
- `FuzzEngine`: Drives the fuzzing loop
- Generates sequences of 1-100 instructions per test case

### 2. Strategies (`strategies/`)
- `InstructionStrategy`: Weighted random selection of:
  - `create_game` (20%)
  - `join_game` (15%)
  - `record_move` (40%)
  - `finalize_game` (10%)
  - `withdraw_expired_wager` (5%)
  - `authorize_session_key` (5%)
  - `delegate_game` / `undelegate_game` (5%)
- `ValidFenStrategy`: Generates valid chess positions
- `InvalidMoveStrategy`: Intentionally bad inputs for negative testing

### 3. Runner (`runner/`)
- Wraps `solana_program_test::ProgramTest`
- Maintains `BanksClient` across instruction sequences
- Tracks account states and balances
- Detects panics, failed assertions, invariant violations

### 4. Invariants (`invariants/`)
Property checks after each instruction:
- Game status consistency
- Turn alternation
- Wager escrow balance matches game state
- ELO changes only on finalized games
- No unauthorized moves accepted

### 5. JSON Controller (`controller/`)
Optional braid-fuzz compatible interface:
- Commands: `init`, `fuzz-step`, `reset`, `get-state`
- Events: `instruction-executed`, `invariant-failed`, `panic`
- Can be disabled with `--no-controller` flag

### 6. Account Funding (`funding.rs`)
Foolproof automatic funding system:
- `TestFaucet`: Built-in `solana_program_test` genesis funding
- `FundedAccountPool`: Pre-funded keypair cache (100+ accounts)
- `AutoRefill`: Detects low balance and refills from master faucet
- `LamportTracker`: Ensures all accounts stay above rent exemption
- No external dependencies - runs in pure test environment

```rust
// Example: Automatic funding
let player = runner.funded_keypair();  // Always has 10+ SOL
let game_id = runner.next_game_id();
// Ready to create_game with wager - no manual funding needed
```

## Account Funding Strategy

**The Problem**: Fuzzing generates 100s of random accounts per run. Manual funding fails.

**The Solution**: 
1. **Genesis allocation**: `ProgramTest::new()` gives genesis account unlimited SOL
2. **Pre-funded pool**: 100 keypairs created at start, each with 100 SOL
3. **Lazy refill**: When account drops below 10 SOL, auto-transfer from master
4. **No external RPC**: Everything in `BanksClient` - no devnet/mainnet needed

**Foolproof guarantee**: Any account returned by `runner.funded_keypair()` is always ready for transactions.

## Dependencies

```toml
[dependencies]
proptest = "1.4"
proptest-arbitrary-interop = "0.1"
anchor-lang = "0.32.1"
solana-program-test = "2.0"
solana-sdk = "2.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

# Local deps
xfchess-game = { path = "../../programs/xfchess-game" }
chess-logic-shared = { path = "../chess-logic-shared" }
```

## Usage Examples

### Native Rust tests:
```rust
#[test]
fn fuzz_game_lifecycle() {
    let mut runner = SolanaRunner::new();
    runner.run_fuzz(1000, |ctx| {
        // 1000 random instruction sequences
        // Asserts all invariants hold
    });
}
```

### Command line:
```bash
cargo run --release -- --iterations 100000 --seed 42 --no-controller
cargo run -- --controller tcp --port 4445  # braid-fuzz compatible
```

### CI integration:
```yaml
- name: Fuzz Contracts
  run: cargo test --package solana-contract-fuzzer -- --nocapture
  timeout-minutes: 30
```

## Fuzzing Targets

1. **Instruction-level**: Individual instruction validation
2. **Sequence-level**: Multi-step game flows
3. **State-machine**: Game status transitions
4. **Economic**: Wager deposits, payouts, rent
5. **Security**: Authorization bypass attempts
6. **Delegation**: ER session key lifecycle

## Success Criteria

- Detects the known AI authority bypass bug (from existing tests)
- Finds edge cases in move validation
- No false positives after 1M iterations
- Runs in under 30 seconds for 10K iterations
- JSON interface passes braid-fuzz basic tests

## Future Extensions

- Coverage-guided fuzzing (afl/cargo-fuzz integration)
- Snapshot/replay for debugging
- Parallel fuzzing across multiple processes
- Minimization of failing test cases
