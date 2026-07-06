# Solana crates

Client- and test-side Solana code. The on-chain program itself lives in
[`programs/xfchess-game/`](../../programs/xfchess-game/); these crates surround it.

| Crate | Purpose | Consumers |
|-------|---------|-----------|
| [`chess-logic-on-chain/`](chess-logic-on-chain/) | **no_std** chess move validation compiled directly into the Solana program behind its `move-validation` feature. Re-exports `nimzovich_engine`'s no_std board/move-gen subset (`CompactBoard`, `validate_and_apply`) so on-chain `record_move` can verify legality inside compute limits. Must never gain a `std` dependency. | Solana program |
| [`solana-chess-client/`](solana-chess-client/) | Client-side transaction builders and RPC helpers for every program instruction (games, sessions, tournaments), plus wallet key handling (`wallet.rs`) and RPC plumbing (`rpc.rs`). The game client consumes it behind `--features solana`; backend signing routes build unsigned transactions with it. | Game client, backend |
| [`er-cu-benchmark/`](er-cu-benchmark/) | Compute-unit benchmark suite for the MagicBlock Ephemeral Rollup move path (`triton-bench` binary). Drives real game flows (`game_flows.rs`, `moves.rs`) against a validator, logs per-instruction CU usage (`cu_logger.rs`), and reports costs (`cost_reporter.rs`, `rpc_bench/`). Used to keep `record_move` within ER budgets. | Benchmarks / CI |

## Program relationship

```
solana-chess-client ──builds txs──► programs/xfchess-game ◄──validates moves── chess-logic-on-chain
                                            ▲
                              er-cu-benchmark (measures CU)
```
