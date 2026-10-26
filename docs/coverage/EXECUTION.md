# Test coverage plan execution

Started 2026-10-26. **The full plan is not complete.** Phase 0 is in progress;
later milestones remain gated on its exit criteria. Existing unrelated workspace
changes have been preserved. Reports are generated locally under `target/coverage`
and each frontend's `coverage` directory; CI uploads them as artifacts.

## Measured before adding behavior tests

| Package / suite | Result | Source line coverage |
|---|---|---|
| wallet-ui | 62 tests passed | 146/711 (20.53%) |
| tournament-admin | 60 tests passed | 208/1858 (11.19%) |
| website Vitest | no unit tests; explicit empty-suite baseline | 0/140 (0%) |
| xfchess-rules | 10 tests passed | 401/510 (78.63%) |
| xfchess-stockfish | 7 passed, 2 intentional ignores | 302/342 (88.30%) |

Frontend denominators include source modules that were never imported by tests.
These are unit-suite results; website Playwright tests are a separate lane. Rust
branch coverage remains unmeasured on stable, represented by null in the baseline.
Engine measurements have been repeated successfully with the canonical coverage
profile. Full application, contract and desktop runs are pending.

## Implemented

- Isolated Rust coverage groups mirroring CI, plus an explicit desktop-feature
  group; JSON, LCOV and HTML exports; a profile that preserves symbols and avoids
  optimizing dependencies during coverage builds.
- V8 coverage configuration and scripts for all three frontends, with lockfiles.
- Per-package/configuration baseline collection and an exact-ratio ratchet,
  comparing PRs against their base commit. Intentional floor changes require the
  `coverage-allow-decrease` label in CI.
- Generated instruction and native IPC matrices. The program currently exposes
  82 instructions (410 cells); 15 cells have reviewed existing assertion evidence.
  Unreviewed source mentions remain candidate counts, not coverage claims.
- Runtime Axum inventory adapter, tested against nested aliases, methods and
  wildcard services. Backend and bridge exporters use actual registration code.
  Runtime HTTP snapshots are pending their native test runs.
- Opt-in backend request observations: method, matched path, status and test
  thread, without concrete URLs, bodies or credentials. Independent inline
  routers still need the observation layer before they contribute runtime hits.
- Shared backend integration harness extracted to `backend/tests/common` and
  used by the existing API suite and the inventory test. It disables startup
  network refreshes/sweeps. A fake chain, upstream mocks and fake time remain M1.
- All 18 ignored tests audited by reason, component owner role and disposition.
  Child-process fixtures and the infinite manual fuzzer are explicitly excluded
  from blanket scheduled execution. One stale native-CPI quarantine is being
  checked for promotion using the SBF harness.
- Explicit live RPC runs and the authority-key integration test now fail when
  prerequisites are missing. The ignore audit is enforced by the Rust safety gate.
- Contributor test policy, a coverage CI workflow, and a manually triggered
  full-suite ten-run soak workflow with failure logs by test name.

Tooling validation: two Node tests for package attribution and ratchet arithmetic,
two Rust tests for runtime-router discovery, matrix freshness checks, baseline
checks against measured reports, Rust extraction/ignore inventory checks, and a
website lockfile install dry run passed.

## Remaining exit criteria

| Milestone | State / remaining work |
|---|---|
| M0 | Finish all native package reports and HTTP snapshots; attach observations to inline routers; review the remaining matrix evidence; run and triage the ten-run CI soak; establish Linux floors. |
| M1 | Chain/RPC and clock injection, scripted fake failures, mock upstream services, JWT helpers and Bevy headless harness. Backend harness extraction and contributor policy are in place. |
| M2 | Money/settlement and on-chain depth work packages, conservation/interleaving properties, mutation targets and manual devnet verification. |
| M3 | Full route-auth classification and behavior/authorization coverage, native bridge tests and secret-hygiene regression. |
| M4 | Netcode model conformance, persistence/restart properties, scripted transport, Loom and migration/query coverage. |
| M5 | Rules/anti-cheat/Swiss targets and headless client state-transition tables. |
| M6 | Frontend flow targets, mobile intent smoke, funded live/nightly and mutation/weekly lanes. |

The soak workflow has been prepared, not run on GitHub from this local working
tree. Live tests need deliberate scheduling, provisioned credentials and funding;
no live funds have been spent during this execution. No manual game UI was opened.

## Product decision

On 2026-10-26 the user selected **Keep current pay-first policy**. WP1 must assert
that wagered 1v1 payouts proceed before anti-cheat analysis. There is no requested
change to payout ordering; the policy regression test remains part of M2.

## Update 2026-10-26 (route-auth, WP2 first slice)

Added `backend/tests/route_auth.rs` (feature `coverage`, 7 tests, green). It classifies every
route the real router registers (Public / Admin / Wallet / Handler / Open) and fails on any
unclassified or stale entry, so a new endpoint cannot ship without an auth decision.

- Admin (every path containing `/admin/`, 60+): missing, wrong, empty key and wallet-JWT-only all give 401;
  a valid key passes on cheap read routes.
- Wallet (37 routes): missing, malformed, wrongly signed, expired token and an API key in place of
  a token all give 401; a valid token passes the gate. Cross-wallet global-session delete gives 403.
  Logout revokes earlier tokens.
- Handler (24 routes): no empty anonymous request succeeds. Per-route body fixtures are still to do.
- Open: 21 credential-less mutating routes are pinned as a reviewed set (social/friends/presence,
  p2p lobby, `broadcast-tx`, external-elo link, `dispute/notify`, `subscribe-node`, legacy
  game-log writes). Shrinking the set is the follow-up; growing it fails a test.

**Defect found and fixed:** `POST /api/actions/admin/tournament/{id}/start` was commented
"admin-only" but mounted under the public `/api/actions` nest with no admin middleware. It returned
transactions signed by the backend authority that start the tournament, to any caller. It now lives
in `blinks_admin_routes()`, mounted behind `require_api_key` + `persist_admin_request` in
`build_app_router`. Removing the layer makes `admin_routes_require_the_admin_key` fail.

Also fixed while running the safety gate: a test module placed before non-test items in
`tauri/src/wallet/http.rs` and a stale import in `backend/tests/e2e_api.rs`. Both were
leftovers of the earlier coverage work. `scripts/ci/check_rust_safety.ps1` is clean.

Still not done: M0 native reports and ten-run soak, M1 seams, M2 money tests, the rest of M3
(per-route Handler fixtures, Tauri bridge behaviour), and M4-M6.

## Update 2026-10-26 (pay-first policy test, Open-route hardening)

- `backend/tests/money_policy.rs` (3 tests) enforces the recorded pay-first decision: the
  anti-cheat modules contain no payout/refund/chain-submission calls, `finalize_game` submits the
  payout before analysis is queued and never consults anti-cheat, and `record_settled_game` only
  queues analysis. These are source-level invariants; a behavioural version needs the M1 fake chain.
- Five credential-less routes now require the caller's wallet token, and the body wallet must match it:
  `POST /api/external-elo/link/{start,confirm}` and `/sync` (anyone could start a link for any
  wallet and set its on-chain rating from their own Lichess account; a link can only be confirmed by
  the wallet that started it), `POST /dispute/notify`, and `POST /{tournament}/{id}/subscribe-node`.
  They are mounted behind `require_relay_or_jwt` (`external_elo_protected_routes`,
  `dispute_notify_routes`, and `tournament_join_routes`). No shipped client calls them, so no client change.
  `route_auth.rs` now has 9 tests; the `Open` set is 15 mutating routes.
- Still Open and deliberately not changed, because shipped clients call them without a token and the
  change cannot be verified without driving the UI: `broadcast-tx`, friends/presence/social poll,
  p2p lobby, `client-events`, and legacy `PUT /game/{id}/moves|chat`.
