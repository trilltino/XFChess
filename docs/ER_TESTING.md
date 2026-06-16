# Ephemeral-Rollups Testing Strategy

How XFChess tests the MagicBlock Ephemeral-Rollups (ER) integration — the
delegate → record-moves-on-ER → undelegate lifecycle in
`programs/xfchess-game/`.

The ER stack can't be fully reproduced on a dev box (the sub-second execution
and `commit`/`undelegate` callbacks are driven by a live MagicBlock validator),
so coverage is **layered**: most logic is pinned by fast in-process tests, and
only the parts that genuinely need a validator are pushed to a devnet runbook.

| Layer | Tooling | Runs in CI? | Covers |
|-------|---------|-------------|--------|
| **L1 — move path** | `solana-program-test` (in-process) | ✅ yes | `record_move` logic: nonce/replay, parent-nonce causal chain, turn enforcement, move legality, session expiry, checkmate/stalemate detection |
| **L2 — account constraints** | `solana-program-test` (in-process) | ✅ yes | delegate/undelegate *guards* that reject before any CPI — e.g. the `address =` constraints on the magic accounts |
| **L3 — full ER lifecycle** | live devnet + ER endpoint | ✅ manual / nightly | real `delegate_game` → `record_move` on the ER → `undelegate_game` → state committed back to base layer |

## Prerequisite

L1/L2 load the **compiled** program, so build it once first:

```bash
cd programs/xfchess-game
cargo build-sbf          # produces target/deploy/xfchess_game.so
```

The test harness sets `SBF_OUT_DIR` to `target/deploy` automatically.

## L1 + L2 — in-process suites

```bash
# build the .so first (above), then:
cargo test -p xfchess-game --test er_move_tests --test er_delegation_tests
```

- [`tests/er_move_tests.rs`](../programs/xfchess-game/tests/er_move_tests.rs) — the ER move path.
- [`tests/er_delegation_tests.rs`](../programs/xfchess-game/tests/er_delegation_tests.rs) — undelegate constraint guards.
- [`tests/common/mod.rs`](../programs/xfchess-game/tests/common/mod.rs) — helpers.

These craft account state directly (a delegated, active `Game` + a
`SessionDelegation`) and submit real instructions built from anchor's generated
client types. The expected next-board is computed with the **same**
`validate_and_apply` the program uses (via `chess-logic-on-chain`), so the test
oracle cannot drift from the on-chain validator.

### Why these can't cover the happy-path delegate/undelegate
`delegate_game` CPIs the MagicBlock **delegation** program and `undelegate_game`
CPIs the **magic** program (`commit_and_undelegate_accounts`). Neither program
exists in `solana-program-test`, so the *successful* lifecycle is an L3 concern.
What L2 verifies is everything that runs **before** those CPIs — the account
validation — including the hardening fix that pins `magic_context` /
`magic_program` to their canonical addresses.

> To extend L2 into a local happy-path test, clone the delegation + magic
> program `.so`s into the test (`ProgramTest::add_program`) from devnet:
> `solana program dump DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh deleg.so`.
> Even then, the ER's asynchronous commit callback isn't reproduced — that
> stays L3.

## L3 — devnet ER lifecycle runbook (manual)

Run against real devnet + the devnet ER endpoint. Costs devnet SOL; flaky by
nature — keep it out of the blocking CI gate, run pre-release / nightly.

1. **Deploy** the program to devnet (`anchor deploy` / `solana program deploy`).
2. **Create + join** a game; **authorize a session key**.
3. **`delegate_game`** (base layer). Confirm the game PDA owner flips to the
   delegation program and `is_delegated == true`.
4. **`record_move`** sent to the **ER endpoint** (not base RPC) — confirm
   sub-second finality and that the board/nonce advance on the ER.
5. **`undelegate_game`** — confirm the ER commits final state back and the PDA
   returns to program ownership.
6. **Assert** the base-layer `Game` reflects every move made on the ER.

The client path that does this lives in
[`src/multiplayer/rollup/magicblock.rs`](../src/multiplayer/rollup/magicblock.rs)
(`delegate_game`, `route_to_er`, `undelegate_game`). A scripted version belongs
in `scripts/` once the magic-router endpoint work (see
`docs/MAGICBLOCK_INTEGRATION.md` §2) lands, so the endpoint isn't hardcoded.

## Gaps / TODO

- **Crank (time-control) path** — `schedule_time_check` / `crank_time_check` are
  scheduled by the magic program and run on the ER; only reproducible at L3.
  Add a devnet step that delegates, lets a clock expire, and asserts auto-flag.
- **Stalemate / 50-move / insufficient-material** — add L1 cases mirroring the
  checkmate test (the handler already branches on these `MoveOutcome`s).
- **L2 happy-path** — optional: clone delegation/magic `.so`s for a local
  delegate→undelegate that stops short of the async ER commit.
