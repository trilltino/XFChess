# Comprehensive Testing Plan

> Status: plan / not yet executed.
> Companion to the test-suite audit. Goal: take the suite from **RED** (does not
> compile) to a green, broad, CI-enforced safety net across all five components.

XFChess is a Cargo workspace with five components (see [CLAUDE.md](../../CLAUDE.md)):
game client (`src/`), backend (`backend/`), Solana program (`programs/`), web
(`web-solana/`), desktop wrapper (`tauri/`), plus shared crates (`crates/`). This
plan covers test strategy for all of them.

---

## 1. Where we are today

### What exists

| Location | Kind | State |
|----------|------|-------|
| `tests/` (game integration) | integration | **Broken — does not compile** |
| `backend/tests/e2e_api.rs` | backend e2e | exists |
| `crates/engine/nimzovich_engine/tests/differential_perft.rs` | perft | exists (gold standard) |
| `crates/shared/swiss-pairing/tests/pairing_fixes.rs` | unit | exists |
| `src/game/{components,resources}/tests.rs` + ~15 inline `#[cfg(test)]` | unit | exist |
| `backend/src/signing/**` (~20 inline `#[cfg(test)]`) | unit | exist |
| `programs/` smoke/security tests + `tests/hardening_tests.ts` | program | partial / misplaced |
| `tauri/src/**/*_tests.rs` | unit | exist |

### What's broken (from the audit) — must fix first

1. `tests/swiss_integration_test.rs` imports `backend::`, `shared`, `axum` — not
   deps of the `xfchess` crate. Wrong crate.
2. `tests/game_flow_tests.rs` uses `shakmaty` (not a dep) and collides with
   `bevy::prelude::Color`. Also tests a third-party lib, not our engine.
3. `tests/p2p_security_tests.rs` is stale — `NetworkMessage` gained fields
   (`agent_id`, `seq`, `parent_version`, …). Duplicate of current in-crate tests.
4. `tests/lib.rs` re-aggregates the above as modules → inherits the failures and
   double-compiles standalone targets.
5. `src/bin/profile_pda.rs` has no `main` without `--features solana`, so
   `cargo test` can't even build the package.
6. `tests/hardening_tests.ts` — TS Anchor test orphaned in a Rust dir.
7. No `[dev-dependencies]`. README documents fictional APIs.

**Principle going forward:** a test belongs in the crate whose code it exercises.
Game-client tests in `src/`/`tests/`, backend tests in `backend/`, program tests
under `programs/`, pairing tests in the `swiss-pairing` crate. No cross-crate
imports from the game crate's integration tests.

---

## 2. Goals & success criteria

1. **Green build:** `cargo test --workspace` compiles and passes with default
   features, and again with `--features solana`.
2. **Layered coverage:** unit (pure logic) → integration (subsystem) → e2e
   (cross-process) for each component.
3. **Deterministic:** no test depends on wall-clock timing, network, live devnet,
   or RNG without a fixed seed. Solana/devnet tests are explicitly gated.
4. **CI-enforced:** every PR runs the suite; a red test blocks merge. No silent rot.
5. **Meaningful:** delete tautological tests (construct-a-struct-assert-its-fields);
   every test can fail for a real regression.
6. **Measured:** coverage tracked (target ≥70% on core logic crates:
   `nimzovich_engine`, `chess-logic-on-chain`, `swiss-pairing`, backend `signing`).

---

## 3. Phase 0 — Stop the bleeding (unbreak the build) — ✅ DONE

Outcome achieved: `cargo test -p xfchess -p shared` is **green** (EXIT 0); backend
build unaffected.

- [x] **Parked** `tests/swiss_integration_test.rs` → `backend/tests/disabled/swiss_integration_test.rs`.
      It was stale against *backend* too (`initialize_pools`, `AppState::new`,
      `swiss_routes` all changed), so it's in a non-compiled `tests/disabled/`
      subdir with a header documenting the Phase-3 rewrite. Not deleted — preserved
      as reference in the correct crate.
- [x] **Deleted** `tests/p2p_security_tests.rs` — duplicate of the current,
      passing `#[cfg(test)]` tests in [src/multiplayer/network/protocol.rs](../../src/multiplayer/network/protocol.rs).
- [x] **Deleted** `tests/game_flow_tests.rs` — it tested `shakmaty`, not our engine.
      (Rewrite against `nimzovich_engine` is folded into Phase 1.)
- [x] **Trimmed** `tests/lib.rs` (not deleted) — it's the only thing that runs the
      `components/` + `resources/` subdir module tests, so it stays, but the lines
      re-including standalone targets (`swiss_integration_test`, `systems_tests`,
      `types_tests`) were removed to stop double-compiles/fan-out.
- [x] **Gated** `profile_pda` **and** `on_chain_benchmark` bins with explicit
      `[[bin]] … required-features = ["solana"]` (both were auto-discovered solana
      bins breaking the default `cargo test`).
- [x] **Moved** `tests/hardening_tests.ts` → `programs/xfchess-game/tests/` for
      `anchor test`.
- [x] **Rewrote** `tests/README.md` to real APIs + conventions.
- [x] **Moved** `tests/networking_tests.rs` → `crates/shared/shared/tests/protocol_tests.rs`
      — it tested `shared::protocol` types the game crate doesn't even import.

### Extra stale-test bugs found and fixed while greening the suite

(These had never run because the suite never compiled.)

- [x] `src/game/time_control.rs` — real bug: `Unlimited.short_label()` produced
      `"0s+0"` instead of the documented `"0+0"`. Fixed the formatter.
- [x] `tests/core_tests.rs` — Bevy 0.18 drift: `init_state` now requires
      `StatesPlugin` after `MinimalPlugins`. Added it.
- [x] `tests/resources/engine_tests.rs` — used `(rank,file)` and never built the
      move cache. Fixed to `(file,rank)` + `rebuild_legal_move_cache()`.
- [x] `tests/systems_tests.rs` — `reset_game_resources` gained a
      `Res<ActiveTimeControl>` param the test didn't insert. Added the resource.
- [x] `src/multiplayer/ui/tx_debugger.rs` — non-compiling doc-test; marked the
      illustrative block `rust,ignore`.

**Result:** 10 integration targets + 225 lib unit tests + 9 `shared` protocol
tests pass. `[dev-dependencies]` were not needed after relocating tests to their
owning crates (the relocations removed the cross-crate imports).

**Gate:** ✅ `cargo test -p xfchess -p shared` green; `cargo test -p backend --no-run` OK.

---

## 4. Phase 1 — Core chess logic (highest value) — ✅ DONE (core)

The heart of the product. The two highest-leverage items are implemented and green.

### `nimzovich_engine` (`crates/engine/`)
- [x] **Canonical perft suite** — `tests/perft_suite.rs`. `perft_known_counts`
      asserts exact node counts (independent ground truth, no reference engine) for
      startpos + Kiwipete + CPW positions 3–6 at CI-fast depths; the slower deep
      counts (startpos d5, Kiwipete d4) live in `perft_known_counts_deep`
      (`#[ignore]`). Perft inherently exercises castling, en passant, promotion, and
      pins, so these cover the "special moves" line too. The existing
      `differential_perft.rs` (vs shakmaty) remains for divergence drilling.
- [ ] (Deferred, lower value) explicit check/checkmate/stalemate assertions,
      FEN round-trip, SAN `san_to_move` ambiguity — perft already covers the
      move-gen surface; these are nice-to-have follow-ups.

### `chess-logic-on-chain` (`crates/`, no_std) — the key safety net
- [x] **Differential validator test** — `tests/differential_validation.rs`. For a
      corpus of positions it computes the engine's legal set and asserts the
      on-chain `is_move_legal` (1) accepts every engine-legal move (no false
      rejects) and (2) rejects every illegal `(from,to)` transition under any
      promotion suffix (no false accepts = no on-chain exploit on staked games).
      Keyed on `(from,to)` so a benign redundant promo suffix isn't flagged.
- [x] `no_std` kept clean: the `std`-only engine helper (`game_from_fen`) is pulled
      in via a **dev-dependency** with `features=["std"]`, so the program's no_std
      build is unaffected (dev-deps don't apply to it).
- Finding: the on-chain validator is lenient about a redundant promotion suffix on
      non-promotion moves (`b1a3q` ≡ `b1a3`). Benign (same board outcome); documented
      in the test.

### `swiss-pairing` (`crates/shared/`)
- [x] Already has `tests/pairing_fixes.rs` (66 tests pass). Extending tie-break /
      full-run coverage remains an optional follow-up.

**Gate:** ✅ perft suite + differential validator pass; full core set green
(`cargo test -p xfchess -p shared -p nimzovich_engine -p chess-logic-on-chain -p swiss-pairing`).

---

## 5. Phase 2 — Game client (`src/`) — ✅ DONE (core)

Kept the genuinely-useful tests; the tautological / non-compiling ones were
removed/relocated in Phase 0.

- [x] **State machine:** `core_tests.rs` covers `AppState` transitions and the
      conditional-system gating (fixed for Bevy 0.18 in Phase 0).
- [x] **Network protocol:** real wire round-trips added — `tests/protocol_roundtrip_tests.rs`
      JSON-round-trips `NetworkMessage`, and crucially **signs → serializes →
      deserializes → still verifies** (the real P2P path; this is what rotted
      before). The `shared` crate gained bincode round-trips for `GameMessage` /
      `LobbyMessage` (`crates/shared/shared/tests/protocol_tests.rs`). Sign/verify/
      tamper unit tests already live in `src/multiplayer/network/protocol.rs`.
- [ ] (Deferred, lower value) board/FEN-sync and replay-final-FEN tests — the
      engine's move correctness is already covered by the Phase-1 perft suite.

---

## 6. Phase 3 — Backend (`backend/`) — ✅ DONE (core)

In-process Axum via `tower`, SQLite, no live devnet. Much was already covered by
the existing `backend/tests/e2e_api.rs` harness (`spawn_app()`); gaps were filled.

- [x] **Auth hardening regression tests** already exist in `e2e_api.rs`:
      `auth_issue_endpoint_is_removed`, `siws_login_then_logout_revokes_token`,
      `login_rejects_stale_timestamp`, `dual_accept_auth_guards_signing_endpoints`,
      `admin_route_requires_api_key` — one per fixed vuln (memory `project_auth_hardening`).
- [x] **Route tests** already present: metrics, blur/think telemetry parity,
      broadcast-delay gating, game history, dispute notify→status.
- [x] **Compliance (`cacf/`)** — added the legally-critical **default-deny**
      tests: restricted jurisdictions (GB/BR/DE/CA) with no record cannot wager;
      non-restricted countries default-allow (`signing/cacf/mod.rs`).
- [ ] (Deferred) tournament-store persistence/prize-math, signing/tx-builder
      assertions, and the parked Swiss e2e rewrite (`backend/tests/disabled/`).
- [ ] (Future) puzzle endpoints — when built (see [PUZZLES.md](../PUZZLES.md)).

---

## 7. Phase 4 — Solana program (`programs/`) — ⚠️ needs a local validator

Anchor tests run via `anchor test` against a local validator — **not runnable in
this environment**, so left as tracked work rather than shipping unverifiable TS.

- The program's most security-critical logic — on-chain move legality — **is
  already guarded** by the Phase-1 differential test (`chess-logic-on-chain` ↔
  `nimzovich_engine`), which runs in normal `cargo test`.
- [ ] `hardening_tests.ts` is now under `programs/xfchess-game/tests/` (Phase 0);
      wire it + smoke/security tests into `anchor test` in CI (nightly, with a
      validator).
- [ ] Negative tests (wrong signer, replayed move, illegal move rejected on-chain,
      double-finalize, fund-after-registration) — require the validator.

---

## 8. Phase 5 — Web (`web-solana/`) & desktop (`tauri/`) — ◑ partial

- [x] **tauri:** extended `utils/crypto.rs` tests — **fixed a never-run broken
      test** (`test_token_validation` asserted a 17-char token valid against a
      ≥32-char rule) and added token-length boundary + `hash_password`
      determinism/salting tests. `config_tests.rs` / `logging_tests.rs` retained.
- [ ] **web-solana:** Vitest + React Testing Library needs an npm/toolchain setup
      (not runnable here) — tracked. Unit-test wallet/connect + tx-building hooks
      with a mocked RPC; add a puzzle-admin page test once built ([PUZZLES.md §9](../PUZZLES.md)).

---

## 9. Phase 6 — CI, tooling, hygiene — ✅ largely DONE

- [x] **CI workflow** already exists: [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
      runs `cargo check --workspace --all-features`, `cargo fmt --check`, clippy,
      `cargo test --workspace`, a dedicated `test-chess-engine` job (unit + the new
      fast perft suite, deep perft `--ignored`, differential perft vs shakmaty), an
      `engine-match-sanity` self-play forfeit check, `test-shared`, and a wasm build.
      The Phase-1 tests added here slot in automatically (perft suite under
      `cargo test -p nimzovich_engine`; differential validator under
      `cargo test --workspace`).
- [ ] Follow-ups: add a `web-solana` lint/test job; tighten clippy to required
      `-D warnings` after a warning cleanup; `anchor test` (nightly, local validator).
- [ ] **Coverage** via `cargo llvm-cov --workspace`; publish the report, fail under
      threshold on the core logic crates.
- [ ] **Determinism guards:** seed all RNG in tests; forbid network in unit tests;
      gate devnet/integration behind `#[ignore]` or a `RUN_DEVNET_TESTS` env flag.
- [ ] **Feature-matrix smoke:** `cargo check --workspace` with and without
      `solana`, so feature-gated code (bins included) never silently breaks the
      default build again — the `profile_pda` failure was exactly this.
- [ ] **Pre-commit hook** (optional): `cargo fmt` + `cargo test --workspace --no-run`
      so a non-compiling test never lands.

---

## 10. Coverage matrix (target end-state)

| Component | Unit | Integration | E2E | CI gate |
|-----------|:----:|:-----------:|:---:|:-------:|
| `nimzovich_engine` | ✅ perft + rules | ✅ FEN/SAN | — | required |
| `chess-logic-on-chain` | ✅ + differential vs engine | — | — | required |
| `swiss-pairing` | ✅ | ✅ full run | — | required |
| Game client `src/` | ✅ state/board/proto | ✅ headless App | — | required |
| Backend | ✅ signing/cacf | ✅ routes/store/auth | ✅ `e2e_api` | required |
| Solana program | ✅ rust logic | ✅ anchor ix | ✅ lifecycle | nightly + PR smoke |
| web-solana | ✅ hooks | — | ⚪ Playwright (opt) | lint+unit required |
| tauri | ✅ config/api | — | — | required |

---

## 11. Execution order (recommended)

1. **Phase 0** (unbreak) — blocking; do first, small.
2. **Phase 6 CI skeleton** — wire `cargo test --workspace` into CI immediately so
   Phase 0's green state is locked in before adding more.
3. **Phase 1** (core chess logic) — highest value per hour; perft suite first.
4. **Phase 3** (backend auth + money paths) — security-critical, money at risk.
5. **Phase 4** (program negative/security tests) — money at risk on-chain.
6. **Phase 2** (client), **Phase 5** (web/tauri) — fill out.
7. Turn on coverage thresholds once the core crates are populated.

The single highest-leverage item is the **perft suite + the
`chess-logic-on-chain` ↔ `nimzovich_engine` differential test**: it guards the
exact logic that, if wrong, lets someone win a staked game with an illegal move.
