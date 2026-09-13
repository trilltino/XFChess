# er-cu-benchmark

Compute-unit benchmark and devnet diagnostics suite for the XFChess Ephemeral Rollup
(MagicBlock) move path. Measures what each program instruction actually costs so
`record_move` and the delegation lifecycle stay within ER budgets.

## Library modules

| Module | Contents |
|--------|----------|
| `game_flows.rs` | End-to-end scripted flows: create → join → delegate → moves → undelegate → finalize |
| `moves.rs` | Move scripts fed to `record_move` during benchmarks |
| `instructions.rs` | Instruction construction for every benchmarked call |
| `cu_logger.rs` | Per-instruction compute-unit capture from transaction metadata |
| `cost_reporter.rs` | Aggregated CU/cost reporting |
| `rpc_bench/` | RPC latency benchmarking |
| `keygen.rs` | Benchmark keypair management (master + child wallets) |

## Binaries

- **`triton-bench`** (`src/main.rs`, plus `bin/triton_bench.rs`) — the main benchmark
  driver.
- **`bin/check_*`** — one-shot devnet inspectors used during integration debugging:
  balances (`check_balances`, `check_child_balance`, `check_children_balances`,
  `check_fee_payer`), program accounts (`check_escrow`, `check_profile`,
  `check_session`, `check_session_owner`, `check_tournament`, `check_tournament_full`,
  `check_shards`, `check_id`, `check_keys`), plus `consolidate_funds`,
  `decode_pubkey`, `find_vps_key`, and `test_auth`.

## Running

Benchmarks run against a live validator (devnet or a local MagicBlock setup) and
spend real lamports from the configured master key — fund the benchmark wallets
first (`keygen.rs` manages the child set), then run `triton-bench`.
