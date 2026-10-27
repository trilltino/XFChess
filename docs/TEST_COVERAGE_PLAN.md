# Test Coverage Plan

Goal: make the test suite trustworthy enough that a green CI run means the money paths,
auth boundaries, netcode and client state machines work, and that a regression in any of them
fails a test before it reaches devnet or players.

Status of this document: written 2026-10-26 from a static survey (test counts, CI config,
file-level inventory). No coverage had been measured at the time of that survey.
Phase 0 is now collecting the first real numbers; targets remain provisional until it lands.

Execution started 2026-10-26 (WP2 route-auth matrix landed the same day and every route now has a credential check as of 2026-10-27; see EXECUTION.md). Measured results, validation and remaining milestone
criteria are tracked in [coverage/EXECUTION.md](coverage/EXECUTION.md). M0 is not
complete until every package has a report and the ten-run flake audit has passed.

---

## 1. What "full coverage" means here

100% line coverage is not the target. It would force tests of rendering, shaders, audio and
`main()` glue that catch nothing, while still missing the bugs that matter (wrong
authorisation, double payout, reordered moves) because line coverage can't see them.

"Full" is defined as four measurable things, in priority order:

1. **Surface coverage (100%).** Every externally reachable entry point has at least one
   success test and one failure/authorisation test: every on-chain instruction, every HTTP
   route (backend and Tauri bridge), every IPC command.
2. **Behaviour coverage (100% of Tier 0/1).** Every money, auth and settlement invariant is
   asserted, with a test that fails when the invariant is broken. Verified by mutation testing
   (Section 6), not by line counts.
3. **Line/branch coverage (ratcheted).** Targets per tier in Section 4. Reported per package,
   never allowed to drop.
4. **Failure-mode coverage.** Disconnects, reorder, duplicate, restart, timeout, RPC outage
   and partial-failure paths are exercised, not only happy paths.

Explicitly excluded from line-coverage targets (list lives in `coverage-exclusions.toml` and
needs a one-line reason per entry): Bevy rendering/camera/picking/debug-visual systems,
audio, shader code, `fn main()` and binary entry points, generated code, the `examples/`
trees, and `tools/cu_benchmark` flows that need a live cluster.

---

## 2. Baseline (what the survey found)

| Area | Inline/other tests | Notes |
|---|---|---|
| `programs/xfchess-game` | ~300 in 43 files | All 72 instructions are referenced by at least one test file (name match; not proof each has a negative test). Strong. |
| `libs/` | ~310 | Rules have perft; anti-cheat 49; braid network crates ~210. |
| `backend/` | ~365 (259 in `signing/`, 38 in `backend/tests/`) | See route matrix below. |
| `src/` client | ~540 | `multiplayer` 163; `states`+`ui`+`rendering`+`input` only 72 across 91 files. |
| `tests/` | 63 | Bevy resource/type tests; `systems_tests.rs` has 1 test; `live_devnet_lifecycle.rs` is `#[ignore]`. |
| `tauri/` | 11 | 35 bridge routes, tests only in `process`, `requests`, `assets`, `mod`, `tournament_admin`. |
| JS | wallet-ui 5, tournament-admin 8, website 4 specs | No coverage tooling configured. |
| `mobile/` | 0 | Kotlin `MainActivity` only. |
| Ignored tests | 18 | Live devnet/RPC; nothing runs them on a schedule. |
| Coverage tooling | none in CI | `cargo-tarpaulin` is installed on the dev machine only. |

**HTTP route matrix (crude).** A script that greps each of the 188 distinct backend route
literals against `backend/tests/` found 172 with no mention. This is an *upper bound* on
what is untested: inline tests in `backend/src/signing` (259) and path building inside
`e2e_api.rs` can cover routes this match misses. Phase 0 replaces it with a real matrix
(Section 3). Notable clusters with no obvious coverage: `/game/*` (delegate, finalize,
undelegate, recover, seat, moves), `/session/*`, `/tournament/{id}/*` (register,
claim-prize, withdraw, round, reseed, manual-pair), `/money-actions*`, `/auth/lichess/*`,
`/external-elo/*`, `/puzzle/*`, `/p2p/*`, `/friends*`, and 40 of the `/admin/*` routes.

**Largest files with zero tests**, which become explicit work items:
`braid-http/src/client/channel.rs` (1,024 lines), `braid_chess/src/outbox.rs` (525),
`multiplayer/network/p2p.rs` (683), `multiplayer/network/vps/game.rs` (531),
`multiplayer/solana/lobby/polling.rs` (480), `multiplayer/solana/tournament/polling.rs`
(442), `multiplayer/rollup/bridge/delegation.rs` (423),
`multiplayer/solana/integration/systems/global_session.rs` (441),
`backend/src/signing/routes/{admin,money_actions,lichess_oauth,external_elo,dispute,wallet,global_session}.rs`.

---

## 3. Phase 0: measure (do this before writing any test)

Without numbers every later phase is guessing. Exit criterion: a coverage report exists for
every package and a tracking file is committed.

**0.1 Rust line/branch coverage**
- Add `cargo-llvm-cov` (works on Windows and Linux; needs `llvm-tools-preview`). Use it in a
  new CI job `coverage`, informational at first (no threshold), uploading `lcov.info` and an
  HTML report as artifacts.
- Run per package group, mirroring the existing CI split so feature sets don't mix:
  `xfchess` + `backend` + rules/stockfish; `xfchess-game` (single-threaded, needs the built
  `.so` and the pass/DLP fixtures); the remaining workspace; `braid-iroh --features proxy`.
- **On-chain program caveat:** `solana-program-test` runs the compiled `.so`, so llvm-cov
  cannot attribute lines in `programs/xfchess-game/src`. Do not chase a line number there.
  Use the instruction matrix (0.3) and mutation testing (Section 6) for the program, and
  count only the host-compiled parts (`chess_logic_on_chain`, state serialisation) with
  llvm-cov through the existing differential tests.

**0.2 JS coverage**
- `vitest run --coverage` (v8 provider) in `tauri/wallet-ui`, `tauri/tournament-admin` and
  `xfchessdotcom`, reported in CI, no threshold yet.

**0.3 Surface matrices (generated, committed, checked in CI)**
Three small scripts under `scripts/coverage/`, each emitting a markdown table and failing CI
when a new entry point has no test row:
- *Instruction matrix:* every `pub fn` in `programs/xfchess-game/src/lib.rs` -> success test,
  wrong-signer test, wrong-account/PDA test, wrong-state test, arithmetic-edge test. Starts
  by recording which of the five cells are filled.
- *HTTP route matrix:* enumerate the real router at runtime (Axum `Router` introspection or
  a test-only route table) instead of grepping source, then record the test that hit each
  route. Instrument via a tower layer in the test harness that logs `(method, matched_path)`
  and dump it at the end of `e2e_api` and the inline route tests.
- *Tauri bridge/IPC matrix:* same for the 35 bridge routes and `invoke_handler` commands.

**0.4 Tracking and ratchet**
- `docs/coverage-baseline.json` holds per-package line% and the matrices' fill counts.
- CI compares the PR's numbers to the baseline and fails on any decrease (ratchet), with an
  explicit override label for intentional removals.

**0.5 Flake and ignore audit**
- Run the full suite 10 times in CI once, record failures by test name, and fix or quarantine
  every flaky test before Phase 1, because new tests on a flaky base make the signal worse.
- List the 18 `#[ignore]`d tests with the reason and owner; decide per test: promote to
  default, move to the nightly job (Section 7), or delete.
- Resolve the two tests noted earlier as silently no-op in CI: they should fail loudly when
  their prerequisite is missing, not pass.

---

## 4. Targets by tier

Targets apply after Phase 0 gives real baselines; numbers are floors to ratchet toward.

| Tier | Scope | Line | Branch | Surface | Mutation score |
|---|---|---|---|---|---|
| T0 Money & settlement | on-chain settlement/escrow/treasury/tournament prize, backend `money_actions`/finalize/claim-prize/withdraw, client wagered settle | 95% (host-compiled code) | 90% | 100% | >= 85% |
| T1 Auth & identity | SIWS, JWT, `RequireWallet`/admin gates, session keys, Lichess OAuth, Tauri bridge `/token`/CORS/Origin | 90% | 85% | 100% | >= 80% |
| T2 State & netcode | braid-http/chess outbox, channel, reorder/dedup, p2p, polling, delegation/ER bridge | 85% | 75% | n/a | >= 70% |
| T3 Game logic | rules, engine integration, anti-cheat, swiss pairing, replay logic, matchmaking | 90% | 80% | n/a | >= 75% |
| T4 Client UI state | menu/lobby/account/tournament-browser state machines, input, game-over flow | 60% | n/a | all state transitions | n/a |
| T5 Front ends | wallet-ui, tournament-admin, website | 80% / 70% / e2e per flow | n/a | all user flows | n/a |
| T6 Mobile | Kotlin wrapper | smoke only | | | |

---

## 5. Work packages

Each package lists the concrete tests to write, where they live, and its exit criterion.
Sizing is relative (S < 1 day, M 1-3 days, L about a week), assuming familiarity with the
harness.

### Prerequisites shared by all packages (do first, size M)

These seams make most later tests possible. Without them the client and backend tests need
live networks.

- **P-1 Chain client seam.** A trait (e.g. `ChainClient`) over the RPC calls the client and
  backend make (get account, simulate, send, confirm, blockhash), with a deterministic
  in-memory fake that supports scripted failure (timeout, `BlockhashNotFound`, rejected
  instruction, partial confirmation). Check first whether `xfchess-client-compat` or the
  existing RPC wrapper already offers this and extend it rather than add a parallel one.
- **P-2 Time seam.** A injectable clock for timeout, clock-on-first-move, reconnect-grace and
  polling logic, so tests don't sleep.
- **P-3 Backend test harness.** A shared builder producing an `AppState` with in-memory/temp
  SQLite, the fake chain client, a `wiremock` server for Privy/Lichess/RPC proxy/mailer, and
  helpers to mint a wallet-signed JWT and an admin token. Today `e2e_api.rs` builds its own;
  extract to `backend/tests/common/`.
- **P-4 Bevy headless harness.** A helper creating a minimal `App` (no window, no render) with
  the plugins a given system needs, the fake chain client, and the fake clock, in
  `tests/common/`. The existing `systems_tests.rs` is the pattern.
- **P-5 Test rules.** Document in `CONTRIBUTING.md`: no `unwrap`/`expect` in tests (the safety
  gate rejects it; return `anyhow::Result`), no live network in default runs, no `sleep`,
  deterministic seeds printed on failure, one assertion theme per test.

### WP1. Tier 0: money and settlement (size L)

On-chain (extend `programs/xfchess-game/tests/`):
- Fill the instruction matrix so every settling instruction (`finalize_game`, timeout claim,
  cancel, resign, draw, tournament prize, `withdraw_treasury`, escrow refund/rollback) has
  all five cells.
- **Conservation property test:** for any sequence of legal instructions, total lamports/tokens
  in (stakes + fees) equals total out (payouts + fees + refunds + rent). `balance_property_tests.rs`
  exists with 5 tests; extend it to cover tournament, global-session and ER-recovery flows,
  with `proptest` generating action sequences and interleavings.
- **Double-settlement:** each terminal instruction called twice, and each pair of terminal
  instructions racing, must settle exactly once. Extend `lifecycle_race_tests.rs`.
- Dust and rounding at every division (fee splits, prize splits for 2/4/N players, ties).
- Zero-move and one-move games for every terminal path (the closed gap must stay closed).

Backend (`backend/tests/money.rs`, new):
- `money_actions` create/list/by-scope/get/idempotency/replay: same request twice yields one
  action; a crash between "record" and "send" is recoverable and never double-sends.
- `/game/finalize`, `/game/delegate`, `/game/undelegate`, `/game/{id}/recover`,
  `/recover_stuck_delegation`, `/tournament/{id}/claim-prize`, `/withdraw`, `/admin/treasury/*`
  against the fake chain with each failure mode from P-1.
- **Policy test for a known gap:** 1v1 payouts currently happen before anti-cheat analysis
  with no clawback. Write the test that states the *intended* behaviour (whatever you decide:
  delay, hold, or accept) so the decision is enforced and visible rather than implicit.

Client (`src/multiplayer/...`, `tests/wagered_settlement.rs`, new):
- Chain-first settlement for resign, agreed draw, checkmate, timeout: UI/game state is not
  marked settled until the chain confirms; a rejected `finalize_game` leaves the game
  recoverable and offers retry; a confirmed-but-lost-response case reconciles from chain
  state on reconnect.
- Reconnect/rejoin: drop at every protocol step (before first move, mid-game, during
  settlement, after settlement), resume, assert no duplicated or lost move and no double
  payout request. Uses P-1 and P-2.
- Casual-rejoin and durable-draw paths from the recent reconnect work.

Exit: T0 matrix 100% filled, mutation score target met on settlement modules, the wagered
client path verified in CI against the fake chain (and once manually on devnet).

### WP2. Tier 1: auth, identity, authorisation (size L)

- **Generic route-auth test (highest value per line of code).** Enumerate the real router (0.3),
  classify every route as public / wallet-auth / admin / relay-secret in an explicit table in
  the test, then for each route send: no credentials, malformed credentials, expired JWT,
  valid JWT for a different wallet, replayed signature. Assert 401/403 for everything not
  public and that mutating routes reject another wallet's identity. A route missing from
  the table fails the test, so new routes can't ship unclassified.
- Per-route success + failure tests for the clusters in Section 2, prioritised: `/auth/*`,
  `/siws-*`, `/ws/auth`, `/session/*`, `/game/{id}/seat*`, `/game/{id}/moves` (per-move
  auth gap noted in the ER audit), `/auth/lichess/*` (state/CSRF, code exchange failure),
  `/external-elo/*` (link confirm requires the proof), `/friends*`, `/p2p/*`.
- Admin surface: all 40 untested `/admin/*` routes get an authorisation-only test (token
  required, wrong token rejected, audit log written) plus a behaviour test for the
  destructive ones (`ban`, `elo-override`, `force-resign`, `treasury/refund`, `ip-ban`,
  `auth/rotate-token`).
- Input validation: oversized bodies, malformed JSON, wrong content type, unicode/length
  limits on usernames, negative and overflow amounts; rate-limit behaviour where one exists.
- **Tauri bridge** (`tauri/src/wallet/tests/` or `tauri/tests/`): 35 routes through the real
  Axum router with `tower::ServiceExt::oneshot`. Cases: foreign `Origin` gets no CORS
  allowance and no token, `/token` and `/pending*` reject unauthenticated callers, path
  traversal on `/wallet-ui/{*path}` returns 400, only one popup at a time, `/status`
  does not leak secrets, shell capability scope tests for the tournament-admin window
  (feature-gated).
- Secret hygiene tests: error bodies and logs never contain RPC URL tokens (regression for the
  earlier leak) via a log-capture test around the RPC error path.

Exit: route-auth test green with 100% of routes classified; auth matrix filled for the
Tauri bridge.

### WP3. Tier 2: state, netcode, persistence (size L)

- **Model-based tests from the specs.** `specs/braid`, `specs/causal`, `specs/settlement` are
  TLA+ models with a Python checker. Generate action traces from the model and replay them
  against the implementation (`braid_chess` and `braid-http`) as conformance tests, so the
  proven properties are also checked against the Rust code.
- **Outbox (`braid_chess/src/outbox.rs`) and channel (`braid-http/src/client/channel.rs`):**
  property tests with `proptest` over arbitrary interleavings of send/ack/drop/duplicate/
  reorder/restart; invariants: every accepted move is delivered exactly once, in order,
  after any number of restarts (persist to a temp dir and reopen).
- **Concurrency:** add `loom` to the lock-heavy pieces (the `parking_lot` shared state in the
  client network layer and the outbox) under a `--cfg loom` test target, limited to small
  models; run on a nightly schedule, not per PR.
- **p2p / vps / polling:** drive with the fake clock and a scripted transport (drop, delay,
  duplicate, partition, peer-join-mid-game). Cover `network/p2p.rs`, `vps/game.rs`, `vps/p2p.rs`,
  `solana/lobby/polling.rs`, `solana/tournament/polling.rs`.
- **Delegation/ER bridge (`rollup/bridge/delegation.rs`) and global session:** every state
  transition and every error branch with the fake chain, including undelegation recovery.
- **Spectator resync** already has 3 tests; add gap, duplicate and late-join cases.
- **DB:** migration tests that apply `backend/migrations` from empty and from the previous
  release's schema, and idempotency (apply twice). Query tests for the hot paths with
  realistic row counts to catch accidental table scans.

Exit: outbox and channel property tests green at 10k cases; loom job green nightly; model
conformance traces replay without divergence.

### WP4. Tier 3: game logic (size M)

- Rules: keep perft; add FEN round-trip property tests, SAN/PGN round-trip, and a
  differential test of `xfchess-rules` against `shakmaty` (or `cozy-chess`) on random legal
  playouts as a dev-dependency only. The on-chain `chess_logic_differential` test already does
  this once; generalise it so client, backend and on-chain rules agree on the same corpus.
- Draw rules: threefold, fifty-move, insufficient material, stalemate, with positions from the
  published test corpora.
- Anti-cheat: golden-file tests over recorded games with known verdicts; threshold boundaries;
  Stockfish unavailable/timeout behaviour must degrade safely (the zombie-engine regression).
- Swiss pairing: property tests (no repeat pairings, colour balance bounds, bye fairness,
  deterministic for a given input), plus tie-break and draw-replay rules from the format split.
- Replay (`game/replay*`), matchmaking, puzzle selection/rating (including the
  double-pay race fixed earlier, as a permanent regression test).

Exit: rules/anti-cheat/swiss at the Tier 3 line and mutation targets.

### WP5. Tier 4: client UI state (size M)

Constraint from your workflow: no launching or clicking through the game. These are headless
`App` tests with P-4, asserting on resources and events, not pixels.

- Extract each screen's pure state transition (menu -> lobby -> in-game -> game-over ->
  settlement, tournament browse/register, account/profile setup, replay enter/exit) so it is
  testable without a window; most files in `states/main_menu/screens` and `ui/` mix view
  and logic. Test the logic, leave the layout.
- Table-driven transition tests: for each state machine, every (state, event) pair has a
  defined result, including "ignored" and "error" cells.
- Replay-exit and AI-gating bugs from the October client audit become permanent regressions.
- Input (`input/board.rs`, 976 lines): selection, legal-move highlighting, promotion,
  premove if present, with the board model and no rendering.
- Pixel/visual checks are out of scope here; if wanted later, a single manual smoke checklist
  in `docs/` is cheaper than automated screenshot tests.

Exit: every state machine has a transition table test; the zero-test files in Section 2
under `states/` and `ui/` either have tests or are listed in `coverage-exclusions.toml`.

### WP6. Tier 5: front ends (size M)

- `wallet-ui`: component and flow tests for login (email, wallet), profile step, transaction
  approval/reject/timeout, popup lifecycle, error states; mock the bridge with MSW.
- `tournament-admin`: every service has a test file already; add the destructive-action
  confirmations and the credential-store path.
- `xfchessdotcom`: keep the four Playwright specs; add the download/install flow, a
  wallet-less visitor path, accessibility checks (`@axe-core/playwright`), and a broken-link
  crawl for the docs and legal pages.
- `vitest --coverage` thresholds set from the Phase 0 baseline.

### WP7. On-chain depth (size M, runs in parallel with WP1)

- **Mutation testing** with `cargo-mutants` on the host-testable parts and, for the program,
  on a nightly job that rebuilds the `.so` per mutant batch (slow; scope to settlement,
  escrow, tournament and authorisation modules). Surviving mutants become new tests.
- **Fuzzing** the instruction handlers with `trident` or an `arbitrary`-driven harness over
  `solana-program-test` for sequences of instructions with random accounts/signers; seed the
  corpus from the existing negative tests.
- **Account-constraint audit tests:** for every `#[derive(Accounts)]` struct, a test that
  substitutes each account with an attacker-controlled one (wrong owner, wrong PDA seeds,
  missing signer, duplicate mutable account). `account_security_tests.rs` has 20; generate the
  rest from the matrix.
- Compute-unit regression snapshot per instruction (the repo already has `cu_benchmark`);
  fail CI when an instruction exceeds its recorded budget by a set margin.
- ABI/IDL snapshot (`abi_snapshot_tests.rs`) extended to the TypeScript/Rust client builders so
  a layout change that breaks clients fails a test.

### WP8. Mobile (size S)

Kotlin wrapper only: one instrumented smoke test (launch, deep link `xfchess://` opens the
intended screen) in CI if an emulator job is acceptable; otherwise a unit test for the intent
parsing. Do not invest further until the mobile client has logic.

---

## 6. Verifying the tests themselves (mutation testing)

Line coverage says code ran, not that a test would notice it breaking. For T0-T3 modules:

- `cargo-mutants` weekly on host-compiled crates (`backend`, `xfchess-rules`,
  `xfchess-anticheat`, `swiss-pairing`, `braid_chess`, `braid-http`), report the surviving
  mutants as a triage list, not a gate at first.
- Gate (fail the PR) only on the T0/T1 modules once their score is above target, scoped to
  changed files so runtime stays reasonable (`--in-diff`).
- Frontend: `stryker` on `wallet-ui` auth/transaction modules only.

---

## 7. CI and process

Split by speed so PR feedback stays fast and the slow, flaky-prone work is scheduled:

| Lane | Trigger | Contents |
|---|---|---|
| Fast | every PR | fmt, clippy, unit and integration tests (existing), route/instruction matrix checks, JS tests, coverage ratchet |
| Contract | every PR touching `programs/**` | existing `.so` build + `xfchess-game` suite, CU snapshot |
| Nightly | schedule | `#[ignore]`d live devnet/RPC tests (retry once, alert on failure rather than block), loom, long proptest (100k cases), fuzz for N minutes, 10x flake run |
| Weekly | schedule | mutation testing, full coverage report published, dependency audit |
| Release | tag | everything above plus the manual devnet lifecycle runbook checklist |

Additional rules:
- Use `cargo-nextest` for the Rust lanes (parallelism, per-test timeouts, retry reporting)
  once the `--test-threads=1` requirement of `xfchess-game` is isolated to that package.
- A bug fix PR must add a regression test; reviewers check for it.
- Any new route, instruction, IPC command or state-machine transition must add its matrix row
  (the CI check enforces this).
- Coverage and mutation reports are published per run; the baseline file is the ratchet.
- Quarantine policy: a flaky test is fixed or deleted within one week; `#[ignore]` requires a
  comment with the reason and an owner, enforced by a grep in the existing safety-gate script.

---

## 8. Sequencing

| Milestone | Contents | Exit criterion |
|---|---|---|
| M0 | Phase 0 (measure) | baseline report + matrices committed, flaky/ignored audit done |
| M1 | P-1..P-5 shared seams | fake chain, clock, backend and Bevy harnesses merged, used by at least one test each |
| M2 | WP1 + WP7 (money, on-chain) | T0 targets met, conservation + double-settlement properties green |
| M3 | WP2 (auth/identity, Tauri bridge) | route-auth test covers 100% of routes |
| M4 | WP3 (netcode, persistence) | outbox/channel properties, model-conformance, loom nightly |
| M5 | WP4 + WP5 (logic, client state) | Tier 3/4 targets met |
| M6 | WP6 + WP8 + nightly/weekly lanes | front ends at target, scheduled lanes live |

M2 and M3 are the ones worth finishing before mainnet. M4-M6 can slip without putting funds
at risk, but M4's outbox work protects the failure players actually notice (lost or duplicated
moves).

---

## 9. Risks and open questions

- **Instrumenting the program.** Line coverage of the on-chain code is not obtainable through
  `solana-program-test`; the plan substitutes the instruction matrix and mutation testing.
  Confirm that is acceptable, or decide whether to add a host-compiled path for the handler
  logic.
- **Refactor cost for WP5.** Splitting logic from view in the large `states/` screens is
  the main cost; scope it per screen and stop when a screen's logic is covered.
- **Mutation testing runtime for the program** may be hours per run; scope to the settlement
  modules first.
- **Live-test dependencies.** Nightly devnet tests need a funded key and a reliable RPC;
  decide where that secret lives and what budget is acceptable (and rotate the Triton token
  noted earlier before adding it to CI).
- **Decision recorded 2026-10-26:** retain the current pay-first policy for wagered
  1v1 games. Payouts proceed before anti-cheat analysis; WP1 must enforce this policy
  with a regression test. User decision: "Keep current pay-first policy".
- **Targets are provisional** until M0. If the baseline is far from a tier floor, move the
  floor for that tier and write the reason here instead of weakening it silently.
