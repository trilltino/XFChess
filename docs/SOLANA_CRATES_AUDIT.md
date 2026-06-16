# Solana Crates Audit — `crates/solana/*`

> **Date:** 2026-06-15
> **Scope:** `chess-logic-on-chain`, `solana-chess-client`, `er-cu-benchmark`, `solana-contract-fuzzer`
> **Trigger:** Suspicion that these crates are outdated / not fit for use.
> **Outcome:** Two are current and load-bearing (kept). The benchmark was stale tooling (modernised). The fuzzer was a non-functional skeleton with a fabricated results file (deleted).

---

## Actions taken (2026-06-15)

- ✅ **Deleted `solana-contract-fuzzer`** (`git rm -r`) — non-functional skeleton; see §4 for why an update wouldn't have been worth it.
- ✅ **Deleted `solana-contract-fuzzer/fuzzer_results.txt`** — fabricated "passing" results for code whose run loop is `todo!()`.
- ✅ **Removed the fuzzer's `exclude` entry** from the root [Cargo.toml](../Cargo.toml).
- ✅ **Modernised `er-cu-benchmark` deps** — switched `anyhow`/`clap`/`tokio`/`serde`/`serde_json`/`solana-client`/`solana-sdk`/`solana-program` to `*.workspace = true` so it can't drift onto a divergent version graph.
- ✅ **Verified `er-cu-benchmark` compiles** (`cargo check -p er-cu-benchmark` → exit 0, only trivial unused-var warnings in `bin/test_auth.rs`). Confirms its `instructions.rs` already tracks the current contract — **no API drift**.
- ✅ **Updated [crates/CLAUDE.md](../crates/CLAUDE.md)** — removed the now-deleted fuzzer as the "excluded crate" example; documented the exclude-vs-workspace-inheritance trade-off.

**Left as-is intentionally:** `chess-logic-on-chain` and `solana-chess-client` are current and load-bearing (§1, §2).

**One open follow-up:** local keypairs under `er-cu-benchmark/keys/` — see §3.

---

## TL;DR

| Crate | Status | Action taken |
|-------|--------|--------------|
| `chess-logic-on-chain` | ✅ Current — core | Kept, untouched. |
| `solana-chess-client` | ✅ Current — core | Kept; optional polish noted (§2). |
| `er-cu-benchmark` | ✅ Modernised | Deps → workspace table; compiles; kept as a member. |
| `solana-contract-fuzzer` | ❌ Deleted | Removed crate + fake results file. |

---

## 1. `chess-logic-on-chain` — ✅ current, untouched

The no_std move validator compiled into the Solana program (`move-validation` feature) and the game client.

- **Used by:** `programs/xfchess-game` ([Cargo.toml:15](../programs/xfchess-game/Cargo.toml#L15) `move-validation = ["chess-logic-on-chain"]`, [Cargo.toml:25](../programs/xfchess-game/Cargo.toml#L25)); root workspace dep ([Cargo.toml:145](../Cargo.toml#L145)).
- **Freshness:** Last touched **2026-06-14** (`f690097f1`, Phase 1 perft + on-chain differential tests). Live test at [tests/differential_validation.rs](../crates/solana/chess-logic-on-chain/tests/differential_validation.rs).
- **Constraint honoured:** `#![no_std]` with `extern crate alloc` and a `std`-only dev-dependency. Matches [crates/CLAUDE.md](../crates/CLAUDE.md).
- **API surface:** thin — re-exports `nimzovich_engine` types + `validation::is_move_legal(fen, uci)`.

Already up to date. No action.

---

## 2. `solana-chess-client` — ✅ current, optional polish

Client-side Anchor instruction builders + RPC fetchers + a `Wallet` trait.

- **Used by:** root workspace dep ([Cargo.toml:141](../Cargo.toml#L141)), pulled into the game client behind the `solana` feature ([Cargo.toml:244](../Cargo.toml#L244)).
- **Freshness:** Last touched **2026-06-11** (`a30e3997a`, auth-hardening pass).
- **Tracks the current contract:** `init_profile`, `create_game`/`join_game` (with `fee_payer`, `platform_fee`, `base_time_seconds`, `increment_seconds`), `record_move` (nonce + optional signature + `parent_nonce`), `finalize_game`, `withdraw_expired_wager`, session-key authorize/revoke, and the **global persistent session** path (`authorize_global_session`, `global_create_game`, `global_join_game`).
- **Note in source:** `create_commit_move_batch_ix` was removed because `CommitMoveBatch` was deleted from the contract — batching now goes through the ER via `record_move`. ([rpc.rs:310](../crates/solana/solana-chess-client/src/rpc.rs#L310))

Current. Optional, not required:

- `system_program_id()` parses the all-1s string at runtime ([rpc.rs:16](../crates/solana/solana-chess-client/src/rpc.rs#L16)); could use `solana_sdk::system_program::ID`.
- `create_finalize_game_ix` takes unused `_player` / `_result` ([rpc.rs:229](../crates/solana/solana-chess-client/src/rpc.rs#L229)).
- Pins `anchor-lang`/`solana-*` directly rather than via the workspace table.

---

## 3. `er-cu-benchmark` — ✅ modernised (was stale)

An Ephemeral-Rollup **compute-unit cost** benchmark (1v1 + Swiss tournament flows) plus 17 one-off `bin/` inspection utilities (`check_*`, `consolidate_funds`, etc.).

**What it's for (and why it's worth keeping):** measures, on the real ER, the **compute units per instruction** (create / join / record_move / finalize) and the **lamport → SOL → GBP cost** per 1v1 game and per Swiss tournament at 8/16/32/64/128/256 players. That validates the on-chain path fits under the CU limit and that the 10p/player platform fee matches real cost. Devnet+ER is the *correct* environment for this (you want real ER costs), so unlike the fuzzer it's architecturally sound.

**Why it was "stale":** it bypassed the workspace dependency table (`anyhow = "1"`, `tokio = "1"`, …), inviting version drift; its instruction layer hand-rolls Anchor wire formats that overlap `solana-chess-client`; and config/keypair paths are baked into constants.

**Fixed now:** deps realigned to the workspace table; verified it still compiles (so `instructions.rs` is *not* drifted from the contract — it already carries `fee_payer`/`platform_fee`/`base_time_seconds`/`increment`).

### Decision: kept as a workspace member (not excluded)

Excluding it from the build would let it bit-rot invisibly **and** an excluded crate cannot use `dep.workspace = true` (no workspace root to inherit from). Since `solana-chess-client` (a regular member) already forces the Solana stack into the default build, excluding the benchmark saves only `solana-transaction-status` + `ephemeral-rollups-sdk`. Keeping it a member means workspace-pinned deps and `cargo build` catches any future contract drift. Net better for a tool we want "fit for use."

### Remaining follow-ups (not blocking)

- [ ] **Consolidate builders:** delete the overlapping hand-rolled builders in `instructions.rs` in favour of `solana_chess_client::ChessRpcClient`, keeping only ER/compute-budget-specific helpers. Not 1:1 (the benchmark has ER `session_create_game`/`delegate` builders the client doesn't expose), so do this carefully — left as follow-up rather than rushed.
- [ ] **Externalise config:** RPC URLs, ER endpoint, `SOL_GBP_RATE`, keypair paths → CLI flags / env (`clap` already pulls the `env` feature via the workspace table).
- [ ] **Keypairs on disk:** `crates/solana/er-cu-benchmark/keys/` holds `er-cu-master.json`, `er-cu-children.json`, **`program-authority.json`**. They are **gitignored** (confirmed not in history), but live keys — especially `program-authority` — in the tree are a footgun. Verify they're disposable devnet keys and consider relocating outside the repo. Relates to [[project_secret_exposure]].
- [ ] Tidy the unused-var warnings in `bin/test_auth.rs`.

---

## 4. `solana-contract-fuzzer` — ❌ deleted

A proptest-based devnet fuzzer that **did not run**. Deleted rather than revived. The evidence and rationale, for the record:

### Why it wasn't fit for use
- **Main loop unimplemented:** `FuzzEngine::run()` was literally `todo!()` — the binary panicked on start.
- **7 of 8 handlers were stubs** returning `ExecutionResult::Skipped` (`join_game`, `record_move`, `finalize_game`, `withdraw_expired`, `authorize_session`, `delegate_game`, `undelegate_game`).
- **Invariant checking was a no-op:** `fetch_game_state()` returned `None`; `check_economic_invariants` was all TODO.
- **The one implemented handler used the OLD contract API:** `build_create_game_ix` serialised only `game_id, wager, game_type(PvP/PvAI)` — the current `create_game` needs `match_type, platform_fee, base_time_seconds, increment_seconds` and a `fee_payer` co-signer, so it would be rejected on-chain.
- **`fuzzer_results.txt` was fabricated** — "523 transactions sent", "Ready for production deployment" — impossible with a `todo!()` run loop.
- **Excluded from the workspace**, so bit-rot was invisible; deps drifted (`ed25519-dalek = 1`, `rand = 0.8`, unpinned `anchor-lang`).

### Why not revive it
Even fully implemented, it fuzzed against **live devnet** — the wrong foundation: slow (network round-trip per case), costs real SOL, and non-reproducible (a `--seed` can't replay against changing chain state). Real Solana fuzzing runs in-process via `solana-program-test` / LiteSVM. Reviving this design would reach a tool you shouldn't run. The invariant space it gestured at (turn alternation, escrow == wager, status validity) is already covered properly by `programs/xfchess-game/tests/security_tests.rs` + `smoke_tests.rs`, the on-chain differential tests, and the TLA+ specs in [specs/](../specs/).

**If property-fuzzing is ever wanted:** build it fresh on `solana-program-test`/LiteSVM, reusing `solana-chess-client` for the wire format.

---

## Cross-cutting notes

- **Single source of truth for the wire format** is `solana-chess-client`. The benchmark's remaining hand-rolled builders are the last duplication; consolidating them is the open item in §3.
- **Default-build weight** is dominated by `solana-chess-client` (a member that unconditionally needs the Solana stack), not by the benchmark.
- **Loose keypairs on disk** under `er-cu-benchmark/keys/` — gitignored; verify disposable, see [[project_secret_exposure]].
</content>
