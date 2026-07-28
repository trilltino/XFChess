# Documentation & Testing Audit Tracking

Tracks the module-by-module pass described in `CONTRIBUTING.md`'s "Documentation
Standards" section: README accuracy, `//!`/`///` (or TSDoc) coverage, test-coverage
gaps, and modularization notes. Phases run bottom-up by dependency: `crates/` first
(nothing else compiles without them), then the Solana program, then the backend,
then the game client, then the two frontends, then the root/docs/ops MD reconciles
against everything below it.

Status values: `todo`, `in-progress`, `done`.

## Phase 1 — crates/

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| crates/shared/backend-types | done | done | no `#[cfg(test)]` in crate — trivial DTO crate, low priority | README described stale backend/frontend relationship; corrected. See Phase 8 finding on duplicate struct. | done |
| crates/shared/swiss-pairing | done | done — `#![warn(missing_docs)]` enabled, 0 warnings | tests already thorough (16 unit tests) | none | done |
| crates/shared/xfchess-anticheat | done (matches code) | partial — lib.rs, error.rs, cross_game/mod.rs done; report/{mod,txt,store}.rs, features/mod.rs, engine/mod.rs still have near-zero doc coverage | no test files found for this crate | | partial |
| crates/engine/nimzovich_engine | done (matches code) | done — rewrote lib.rs to drop historical narrative and a stale module list marking `board`/`move_gen`/`evaluation`/`hash` as "(TODO)" when fully implemented, and replaced an obsolete "Phase 1/2/3" AI-integration tutorial with a pointer to the real `src/game/ai/` integration. Submodules (1384 doc lines / 425 pub items) already thorough. | perft suites + nimzovich-uci match testing already cover this well | | done |
| crates/engine/nimzovich-uci | done (matches code) | done — already accurate, no changes needed | covered by cutechess-cli match testing per README | | done |
| crates/solana/chess-logic-on-chain | done (matches code) | done — added crate `//!` | has 4 inline tests already | | done |
| crates/solana/solana-chess-client | done (matches code) | done — added crate `//!` + const docs; rpc.rs (22/23) and wallet.rs (3/4) already well covered | | | done |
| crates/solana/er-cu-benchmark | done (matches code) | already good — crate `//!` + full item docs present | standalone benchmarking tool, not app-linked; low priority | | done |
| crates/zarathustra_net/braid-core | done (matches code) | done — crate doc rewritten to drop changelog-style "historical...removed" language | | | done |
| crates/zarathustra_net/braid-http | done (matches code) | done — added crate `//!` + module docs; excellent README already | | | done |
| crates/zarathustra_net/braid-iroh | done (matches code) | done — added crate `//!` + `BraidIrohState` field docs | | | done |
| crates/zarathustra_net/braid_chess | done (matches code) | already excellent, no changes needed | | | done |
| crates/zarathustra_net/iroh-h3 | done (matches code) | already excellent (`#![deny(missing_docs)]`, fully documented) | unused/experimental per its own README | | done |
| crates/zarathustra_net/iroh-h3-axum | done (matches code) | already excellent (`#![deny(missing_docs)]`) | unused/experimental per its own README | | done |
| crates/zarathustra_net/iroh-h3-client | done (matches code) | already excellent (`#![warn(missing_docs)]`) | unused/experimental per its own README | | done |
| crates/zarathustra_net/xfchess-braid-server | done (matches code) | already excellent, no changes needed | | | done |

Excluded (vendored, not first-party): `crates/zarathustra_net/iroh-gossip`.

## Phase 2 — programs/xfchess-game/src/

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| account_ix/ | done (matches code) | done — all 9 files (profile, fee_vault_ix, friends_ix, global_session_ix, link_external_elo, profile_init, session_guards, set_username, treasury, withdraw) now have struct+handler docs; friends_ix/global_session_ix/treasury were already excellent | no inline `#[cfg(test)]` | | done |
| common/ | done (matches code) | already excellent, no changes needed | | | done |
| crank_ix/ | done (matches code) | done — crank_time_check.rs item docs added; schedule_time_check.rs already good | | | done |
| delegation_ix/ | done (matches code) | done — struct docs added to delegate.rs, session.rs, undelegation.rs | | | done |
| elo/ | done (matches code) | done — rating.rs fn docs added; glicko2.rs already documented + tested; README already flags the "glicko2 filename, plain-Elo formula" naming quirk | glicko2.rs has tests already | | done |
| game_ix/ | done (matches code) | done — 7 live files documented (create, join, cancel, resign, timeout, global_create, global_join, common); finalize.rs already excellent. **Found dead code**: record.rs + record_move.rs are not declared in mod.rs, not compiled — logged in legacy-cleanup-audit.md, deliberately left undocumented | no inline `#[cfg(test)]` on live files | record.rs/record_move.rs orphaned — see Phase 8 | done |
| governance_ix/ | done (matches code) | done — dispute.rs, resolution.rs, resolve.rs, claim_stale_dispute.rs all documented | | | done |
| lifecycle/ | done (matches code) | done — clock.rs, guards.rs, transitions.rs, terminal.rs, settlement.rs all documented | guards.rs, clock.rs untested at unit level | join_waiting_game duplicated inline in global_join.rs — see Phase 8 | done |
| magicblock/ | done (matches code) | done — crank.rs, delegation.rs, routing.rs documented | routing.rs untested (const only, low priority) | | done |
| moves_ix/ | done (matches code) | done — apply.rs and record.rs (the real, compiled one) documented | apply_recorded_move untested at unit level despite being the core move-validation entrypoint | | done |
| state/ | done — corrected treasury_vault.rs table row | done — player_profile.rs (1→full), dispute.rs, tournament_match.rs, player_session.rs, treasury_vault.rs documented; game.rs got a `phase()` doc; friendship.rs/global_session.rs/username_record.rs/tournament_session.rs/tournament.rs/platform_fee_vault.rs already had good ratios | tournament_session.rs, global_session.rs, game.rs already tested | rd/volatility dead fields + TreasuryVault dead type — see Phase 8 | done |
| tournament_ix/lifecycle | partial | done: initialize.rs, initialize_shards.rs (all 3 tiers), shards.rs (top-level helpers). Not revisited: cancel.rs, close_tournament.rs, start.rs, initialize_escrow.rs (already had reasonable ratios per density scan) | | | partial |
| tournament_ix/matches | partial | done: guards.rs, initialize_match.rs. Not revisited: advance_round.rs, record_result.rs, record_swiss_result.rs (moderate ratios already) | round_bitmap.rs already tested | | partial |
| tournament_ix/prizes | partial | done: ledger.rs (0→full). Not revisited: claim_prize.rs, distribute.rs, fund_prize.rs, fund_sol_prize.rs (moderate ratios already) | | | partial |
| tournament_ix/registration | todo | not revisited this pass — register.rs/leave.rs already had moderate ratios (9/12, 6/10) | | | todo |
| tournament_ix/session | todo | not revisited this pass — authorize_tournament_session.rs already well documented (16/20); session_create_game.rs, session_join_game.rs moderate | | | todo |

## Phase 3 — backend/src/

Phase 3 was completed exhaustively (every file in backend/src/ opened and
checked, not just density-scanned) after an explicit request to finish
backend/ before moving to Phase 4. Most files were already well documented;
this table records what changed and what was verified as already adequate.

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| db/ | good already | mod.rs got a module doc; repository.rs/sessions.rs/schema.rs already adequate | | | done |
| infrastructure/ | fixed stale CORS claim | mod.rs, database.rs, ngrok.rs, router.rs, tasks.rs, auth_middleware.rs all already excellent | | | done |
| signing/ (top-level files) | n/a | auth.rs, config.rs, identity.rs, feepayer.rs, anticheat_enqueue.rs, tournament_gossip.rs already excellent. elo_cache.rs, linkage.rs got missing docs + a Glicko-2→Elo correction. auth_ws.rs and tee_relayer.rs documented as live-but-fake (see Phase 8). ws_subscriber.rs cross-referenced to tasks/mod.rs's existing explanation. mod.rs (AppState, 73 pub items) already adequately documented. | | auth_ws.rs, tee_relayer.rs findings — see Phase 8 | done |
| signing/blinks/ | fixed stale "funding" feature claim | mod.rs, anti_cheat.rs, pda.rs, chains.rs, onboarding.rs, core.rs, routes.rs already excellent, confirmed live at `/api/actions`. funding.rs documented as dead (see Phase 8). | | funding.rs dead — see Phase 8 | done |
| signing/cacf/ (brazil, canada, germany, uk, mod, types) | corrected — not the live enforcement path | all 6 files already fully documented and unit-tested | | **entire module unused — see Phase 8, highest-stakes finding in this audit** | done |
| signing/p2p_relay/ | n/a | mod.rs, state.rs, routes.rs, types.rs all already good — types.rs's raw ratio was misleading (self-descriptive plain-data fields) | | | done |
| signing/routes/ (23 files) | fixed stale debug-endpoint path | admin.rs (947 lines, was the sparsest file in the repo) got a full module doc + docs on its 5 highest-risk handlers. debug.rs, archive.rs, kyc.rs, dispute.rs, relayer.rs, mod.rs got missing docs. auth.rs, tournament.rs, wallet.rs, global_session.rs, anticheat.rs, casual_games.rs, chat.rs, external_elo.rs, history.rs, identity.rs, lichess_oauth.rs, mailer.rs, main.rs, matchmaking.rs (+handlers/state), puzzle.rs, rates.rs, rpc_proxy.rs all confirmed already excellent. | | relayer.rs is a live duplicate stub of tee_relayer.rs — see Phase 8. debug.rs's endpoint is a stub — see Phase 8 | done |
| signing/social/ | n/a | friends.rs (3→full) and presence.rs (3→full) fully documented | | separate from on-chain Friendship PDA — documented | done |
| signing/solana/ | n/a | mod.rs, rpc.rs, transactions.rs already excellent. debug.rs's stub documented precisely (see Phase 8). instructions.rs (114 doc lines) confirmed already thorough. | | debug_transaction is a stub — see Phase 8 | done |
| signing/storage/ | n/a | vault.rs (KycStatus variants added), session.rs, tournament.rs already excellent | | | done |
| signing/swiss/ | n/a | mod.rs, service.rs, handlers.rs already excellent; orchestrator.rs cross-referenced to tasks/mod.rs's existing "not wired up" explanation | | | done |
| tasks/ | n/a | mod.rs already excellent (self-documents 2 known-unwired subsystems). anticheat_worker.rs, matchmaking.rs, queue.rs, settlement_worker.rs, tournament_scheduler.rs already excellent. archiver.rs got a struct doc. **fee_claimer.rs had a real bug**: a `use` statement split its module doc comment into two disconnected fragments — fixed. | | | done |
| telemetry/ | n/a | mod.rs, logging.rs, metrics.rs, worker_metrics.rs already excellent. middleware.rs's `extract_context_middleware` documented as an unused, unregistered no-op instead of looking like active logic. | | | done |
| bin/ | n/a | convert_keys.rs got a module doc (was bare); import_puzzles.rs already excellent; tournament_admin.rs/vps_admin.rs already covered by bin/README.md | | | done |

Resolved in Phase 9: `backend/tests/disabled/swiss_integration_test.rs` (parked,
cited a nonexistent `docs/plans/comprehensive-testing.md`) was rewritten and
moved to `backend/tests/swiss_tournament_e2e.rs` — see the Phase 9 table below.

## Phase 4 — src/ (game client)

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| core/ | good already | done — README accurate; resources.rs already well covered (self-evident methods left undocumented per standard) | | | done |
| game/ (+ ai, components, resources, sync, systems) | good already | done — 55 files, already very well documented overall (1644 doc-lines/690 pub items). Fixed pending.rs (0 docs), added accuracy to sync.rs (GameSyncPlugin is a registered no-op, misleadingly commented as "Add network sync plugin" at its call site in plugin.rs — fixed both). mod.rs files (game, ai, resources, systems) already have thorough module docs. | | GameSyncPlugin is a dead no-op, still registered — see Phase 8 | done |
| engine/ | n/a | already excellent, no changes needed | | | done |
| multiplayer/ (+ network, rollup, solana, tournament, ui, wager_state) | n/a | 61 files, already well documented overall (1099 pub/1103 doc). network/vps/social.rs (45 pub/2 doc) got full docs. rollup/mod.rs got a module doc. Found and documented a 3-piece dead cluster around hot-wallet game creation (solana/rpc.rs orphaned, integration/rpc.rs unused, integration/systems.rs's SolanaRpc+3 unregistered systems) and rollup/mvp_plugin.rs (unused). | | 5 dead-code findings — see Phase 8 and legacy-cleanup-audit.md | done |
| rendering/ (+ board, camera, effects, pieces) | n/a | density-scanned (181 doc-lines/138 pub-items across 19 files) — already good, no file stood out as a real gap | | | done |
| solana/ (+ core, multiplayer, program_interface, session, wallet) | n/a | program_interface/instructions.rs confirmed heavily used and well documented (141 doc-lines/40 pub). Found solana/errors.rs + core/errors.rs (duplicate unused XfChessError) and constants.rs (unused, PLAYER_SEED doesn't match real on-chain seed) — documented as dead. solana/multiplayer/ already known-legacy per legacy-cleanup-audit.md item 1. | | 3 more dead-code findings — see Phase 8 | done |
| states/ (+ main_menu) | n/a | already excellent (315 doc-lines/165 pub-items) | | | done |
| ui/ (+ account, game, menus, styles, system_params, tournament) | n/a | density-scanned (307 doc-lines/493 pub-items across 27 files) — reasonable overall; menus/multiplayer_menu.rs's low ratio matches its already-known legacy/unreachable status (legacy-cleanup-audit.md item 7), left alone rather than documented as if live | | | done |
| input/ | n/a | already good, small file | | | done |
| presentation/ | n/a | already good, small file | | | done |
| puzzle/ | n/a | already excellent — thorough module doc + item docs throughout | | | done |
| xf_animate/ (+ games) | n/a | density-scanned (27 doc-lines/74 pub-items) — reasonable, no file stood out as a real gap | | | done |
| bin/ | fixed stale README claim | README's "Gotchas" section claimed these tools share constants with `src/solana/constants.rs` — but that file is confirmed unused (see solana/ row above); each bin tool actually hardcodes its own local PROGRAM_ID/seed constants. Corrected. | | | done |

## Phase 5 — tauri/

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| tauri/src/ (+ services, types, utils, windows) | already excellent | done — found and documented a large dead scaffold layer (types/*, services/auth.rs, services/config.rs, services/ipc.rs::IpcServer); real code (main.rs, services/ipc.rs's actual commands, windows/, utils/) already well documented | | substantial dead-code cluster — see Phase 8 | done |
| tauri/tournament-admin/src/ | already excellent | spot-checked services/api.ts, hooks/useAuth.tsx — already well documented (JSDoc + explanatory comments) | no test infra at all | | done |
| tauri/wallet-ui/src/ | already excellent | App.tsx already thoroughly commented. Fixed 7 instances of a corrupted em-dash character (mojibake, "�") in comments — encoding artifact, not a content issue. | no test infra at all | | done |
| tauri/viz/ | already excellent (per README) | not individually spot-checked beyond the README given time budget; excluded from the root Cargo workspace and self-contained | no test infra at all | | partial |

## Phase 6 — xfchessdotcom/src/

| Module | README | Doc comments | Test gaps | Modularization notes | Status |
|---|---|---|---|---|---|
| components/ | already good | not individually spot-checked beyond StructuredData.tsx/PlatformIcons.tsx density (already reasonable) | | | partial |
| hooks/ | already good | done — useWalletUsdBalance.ts's two exported functions got JSDoc | | | done |
| lib/ (+ api, seo) | already good | spot-checked client.ts, auth.ts, api.ts (facade, confirmed legitimate not a duplicate) — all already excellent | | | partial |
| pages/ | already good | ProfileViewer.tsx: fixed 6 corrupted mojibake characters (5 em-dashes, 1 arrow, 1 UI-visible loading indicator) — real bug, not just docs. Other pages not individually spot-checked given time budget; density scan showed simple page components with self-evident names, matching this codebase's existing convention of untitled single-purpose page files. | | | partial |

Phase 6 was a representative spot-check (central/foundational files: api/client.ts,
api/auth.ts, the api.ts facade, useWalletUsdBalance.ts, ProfileViewer.tsx) rather
than exhaustive file-by-file coverage, given the scope remaining in Phases 7-9.
No dead-code clusters found here, unlike Phases 2-5 — this frontend appears to be
a single, current implementation without parallel/abandoned versions.

## Phase 7 — root/docs/ops MD

| Path | Status | Notes |
|---|---|---|
| README.md (root) | not reached | |
| CLAUDE.md (root) | done | fixed the `/api/debug/tx/{signature}` path (was documented as `/api/debug/transaction/:signature`) while auditing backend/src/signing/solana/debug.rs |
| CONTRIBUTING.md | done | Documentation Standards section added (Phase 0) |
| SECURITY.md | not reached | |
| MAGICBLOCK.md | not reached | |
| agent-blueprint.md | resolved | **Not XFChess documentation** — a separate, unrelated planning doc for a different coding-agent tool project ("repo-agent"), dated 2026-07-26, that happens to live at the repo root. Left as-is; out of scope for this audit. |
| backend/CLAUDE.md | done | verified accurate during Phase 3 |
| xfchessdotcom/CLAUDE.md | not reached | |
| crates/CLAUDE.md | done | fixed two stale "Used by" rows: `iroh-h3`/`-axum`/`-client` (said Backend, actually unused anywhere) and `backend-types` (said "Backend, web frontend", actually only the game client depends on it — the backend hand-maintains a parallel struct instead, per the Phase 1 finding) |
| docs/README.md | done | added the AUDIT_TRACKING.md entry (Phase 0) |
| docs/adr/* | not reached | |
| docs/architecture/* | done | xfchess-game-crate.md and magicblock-game-lifecycle.md both verified accurate against the actual Phase 2 code audit — no changes needed |
| docs/plans/* | partial | treasury-payout-and-close-tournament-fixes.md's status line was stale ("Not yet deployed") — corrected to reflect it's been live on devnet since the 2026-07-16 upgrade. global-session-flow-fix-plan.md is the user's active WIP (open in their IDE) — left untouched. Other 6 plan files not individually re-verified given time budget. |
| docs/runbooks/* | done | **runbooks/README.md's index was missing 3 of the 7 actual runbook files** (game-settlement.md, magicblock-lifecycle-devnet.md, tournament-lifecycle.md) — fixed |
| ops/README.md | todo | |
| ops/docs/* | todo | |

## Phase 8 — Modularization findings

Populated during Phases 1–6. Every module was reviewed for the "does this
file do one thing" question during its pass; nothing structurally alarming
turned up beyond the items below. The consistent pattern across this audit
is **not** poor modularization — it's **parallel, unfinished implementations
sitting alongside the real, live ones**, almost always in the Solana/wallet
integration layers, almost always self-consistent within themselves (well
documented, sometimes unit-tested) but never wired to anything.

### Punch list, by priority

**Needs a product/legal decision, not just cleanup:**
1. **`backend/src/signing/cacf/*`** — a complete, tested, unused compliance
   implementation sitting next to the simpler one actually gating wagers.
   Given the legal weight of CACF compliance for the 4-country release,
   confirm the live path is sufficient before deleting the richer one.
2. **`backend/src/signing/tee_relayer.rs` + `routes/relayer.rs`** — four live
   HTTP endpoints (`/tee/pubkey`, `/tee/attestation`, `/relayer/pubkey`,
   `/relayer/attestation`) return hardcoded fake strings to any caller today.
   Decide: finish one real implementation, or remove both.
3. **`backend/src/signing/auth_ws.rs`** — `/ws/auth`'s post-handshake message
   loop returns fabricated data. Decide: finish it, or drop to wallet-only auth.

**Zero-risk deletions (not compiled / never instantiated on-chain) — see `docs/legacy-cleanup-audit.md`:**
4. `programs/xfchess-game/src/game_ix/{record.rs,record_move.rs}` (orphaned)
5. `programs/xfchess-game/src/state/treasury_vault.rs`'s `TreasuryVault` type (never instantiated)
6. `src/multiplayer/solana/rpc.rs` (orphaned)

**Safe to delete after a quick "confirm no caller" check (compiled, unreferenced):**
7. `src/multiplayer/solana/integration/rpc.rs` + the dead cluster in `integration/systems.rs`
8. `src/multiplayer/rollup/mvp_plugin.rs`
9. `src/solana/errors.rs`, `src/solana/core/errors.rs`, `src/solana/constants.rs`
10. `tauri/src/types/*` (whole directory), `services/auth.rs::AuthState`, `services/config.rs`
11. `backend/src/signing/blinks/funding.rs`

**Needs a real migration plan (can't just delete — live on-chain data):**
12. `state::PlayerProfile::{rd, volatility}` — dead fields, but removing them
    changes the byte layout of already-deployed accounts.

**Small correctness/duplication fixes (low risk, quick):**
13. `game_ix::global_join::handler` should call `lifecycle::transitions::join_waiting_game`
    instead of re-implementing it inline.
14. `src/game/sync.rs::GameSyncPlugin` — registered no-op; comment at its call
    site was actively misleading (fixed). Consider removing the registration.

**Already fixed as doc-only corrections during this audit** (stale paths, stale
"used by" claims, mojibake, a split doc comment) — see the detailed table below
and Phase 7's tracking row for specifics; no further action needed on these.

### Detailed table

| Location | Finding | Resolution |
|---|---|---|
| `crates/shared/backend-types::tournament::TournamentSummary` vs. `backend/src/signing/routes/tournament.rs::TournamentSummary` | Two independently maintained struct definitions for the same wire format; backend doesn't depend on the shared crate at all despite the crate's name/original README implying it does. Only the game client uses the shared crate. | Deferred — either make the backend depend on `backend-types` for this struct (removes hand-sync risk) or rename/re-scope the crate to reflect that it's actually a game-client-side wire-type crate, not a backend↔frontend shared crate. Needs a decision, not a doc-only fix. |
| `crates/CLAUDE.md` crate inventory table vs. `crates/zarathustra_net/iroh-h3{,-axum,-client}/README.md` | `crates/CLAUDE.md` lists `iroh-h3`, `iroh-h3-axum`, `iroh-h3-client` as "Used by: Backend"; all three crates' own READMEs say in bold "Currently NOT actively used in the chess program" and recommend considering removal. | Fix in Phase 7 when reconciling `crates/CLAUDE.md` — correct the "Used by" column to match the per-crate READMEs (experimental/unused), or verify with a dependency check whether backend actually references them before deciding which side is stale. |
| `programs/xfchess-game/src/game_ix/record.rs` and `record_move.rs` | Not declared in `game_ix/mod.rs` — not reachable from any `mod` statement, so not compiled at all. Duplicate/legacy of the real move-recording path in `moves_ix/record.rs`. `record.rs`'s handler body reads as an abandoned draft. | Logged in `docs/legacy-cleanup-audit.md` as item 4 (Safe Remove, zero risk). Left undocumented here deliberately — adding polished doc comments to dead code would misrepresent it as live. |
| `lifecycle::transitions::join_waiting_game` vs. `game_ix::global_join::handler` | `game_ix::join` calls the shared `join_waiting_game` helper to transition `WaitingForOpponent` → `Active`; `game_ix::global_join` reimplements the identical field mutations inline instead of calling it. Two copies of the same transition logic that must be kept in sync by hand. | Deferred — have `global_join::handler` call `join_waiting_game` instead of duplicating it. Small, low-risk refactor; not done as part of this doc-only pass. |
| `state::PlayerProfile::{rd, volatility}` | Two `f64` fields reserved for an abandoned Glicko-2 rating attempt (per the removed comment "Old simplified Glicko-2 implementation removed"). Never read or written anywhere in the program — confirmed via grep. | Not a simple deletion: removing fields from a live `#[account]` struct changes the on-chain byte layout and breaks existing deployed `PlayerProfile` accounts. Needs a real migration plan (or leave as permanently-zero padding), not a doc-only fix. Documented in place as unused rather than removed. |
| `state::TreasuryVault` (whole struct) | A per-country typed treasury account (`seeds = [TREASURY_VAULT_SEED, country]`) that is never constructed, deserialized, or referenced by any instruction. Every real treasury reference in the program uses a single global untyped `SystemAccount` at `seeds = [TREASURY_VAULT_SEED]` (no country component) instead. | Unlike the `PlayerProfile` fields, this type was never instantiated on any deployed account, so unlike a live-struct field removal, deleting the type is zero-risk (no on-chain data of this shape exists to break). Logged in `docs/legacy-cleanup-audit.md` as a Safe Remove candidate. |
| `backend/src/signing/auth_ws.rs::handle_auth_websocket` | Live, mounted at `/ws/auth` (`signing/mod.rs`). The JWT handshake is real, but the message loop that runs after auth returns a hardcoded fake payload (`"token": "updated_token", "wallet_pubkey": "updated_pubkey"`) for every client message instead of any real state. Since the wallet, not this JWT layer, is the real gameplay identity (per `backend/CLAUDE.md`), this may be low-stakes, but it's a live endpoint doing nothing real past the handshake. | Deferred — needs a product decision (finish the sync loop, or remove it and have clients rely on wallet-based auth only) rather than a doc-only fix. Documented in place so it isn't mistaken for working. |
| `backend/src/signing/tee_relayer.rs` **and** `backend/src/signing/routes/relayer.rs` | Two separate, parallel stub subsystems, both mounted live in `signing/mod.rs` (`.merge(relayer::routes())` and `.merge(tee_relayer::routes())`). `tee_relayer.rs`'s `TEERelayer::{sign_and_submit, get_public_key, get_attestation_quote}` are unimplemented and confirmed unused (real signing goes through `signing::solana::sign_and_submit`); its `GET /tee/pubkey` / `GET /tee/attestation` routes return hardcoded placeholder strings. `relayer.rs`'s `GET /relayer/pubkey` / `GET /relayer/attestation` do the exact same thing under a different path prefix — four live endpoints total, all fake. | Deferred — either implement one real relayer-identity/attestation integration and delete the other, or remove both if this feature was abandoned. Two parallel copies of the same unfinished feature is itself worth resolving, not just documenting. |
| `backend/src/signing/blinks/funding.rs` | MoonPay/Transak/Banxa URL-generation helpers with zero callers anywhere in the codebase (confirmed via search). Superseded by the project's actual USDC/Privy-based fiat on-ramp direction, which lives elsewhere and isn't in this crate. `blinks/mod.rs`'s module doc listed this as an active feature ("Wallet funding integration") — corrected. | Dead code — safe to delete in a follow-up cleanup pass (not done here; this was a doc-only pass). Not added to `legacy-cleanup-audit.md` since, unlike the program-side findings, removal here doesn't need on-chain-layout risk analysis — just confirm no external client calls these HTTP-adjacent helpers before deleting. |
| **`backend/src/signing/cacf/*` (all 6 files: mod.rs, types.rs, uk.rs, brazil.rs, canada.rs, germany.rs)** | A complete, fully unit-tested, per-country CACF compliance implementation (`CacfComplianceManager` + per-country structs with KYC/tax-ID/reporting fields) that is confirmed **never constructed or called anywhere** in the backend. The wager-gating logic actually live in production is a separate, independent implementation in `storage/vault.rs` (`cacf_can_wager`/`save_cacf`/`load_cacf_status`, SQLite `cacf_compliance` table), called from `routes/kyc.rs`. Both independently encode the same default-deny rule for GB/BR/DE/CA. | **Highest-stakes finding in this audit** given the legal significance of CACF compliance for the 4-country release. Not a doc-only fix — needs a product/legal decision: is `storage/vault.rs`'s simpler implementation considered sufficient (in which case `signing/cacf/*` is safe to delete), or was the richer per-country tracking in `signing/cacf/*` (HMRC/CPF/SIN/tax-ID fields, monthly reporting) actually intended to be live and never got wired in? Documented in place; not deleted or wired in as part of this doc-only pass. |
| `src/multiplayer/solana/rpc.rs`, `integration/rpc.rs`, `integration/systems.rs`'s `SolanaRpc`/`setup_solana_system`/`handle_game_transactions`/`handle_tournament_transactions` | A cluster of three separate dead pieces around direct hot-wallet on-chain game creation: `solana/rpc.rs` is orphaned (not declared in `solana/mod.rs`, not compiled at all — same class as the earlier `game_ix` finding). `integration/rpc.rs`'s `initiate_game_on_chain`/`join_game_on_chain`/`prepare_final_game_state` are real, non-stub implementations but confirmed never called anywhere. `integration/systems.rs` defines a third, differently-scoped `SolanaRpc` plus three functions that reference it, but none of the three are ever registered as Bevy systems (`SolanaIntegrationPlugin::build` never calls them) and two of the three bodies are themselves placeholders. | Deferred — this looks like an abandoned earlier "hot wallet signs directly" architecture superseded by the current backend-builds-unsigned-tx model. Safe to delete `solana/rpc.rs` immediately (zero risk, not compiled). `integration/rpc.rs` and the three dead functions in `integration/systems.rs` are technically compiled but unreferenced — confirm no external caller before deleting. |
| `src/solana/errors.rs`, `src/solana/core/errors.rs`, `src/solana/constants.rs` | Two near-duplicate `XfChessError` enums (unused anywhere) and a constants file (`SOLANA_PROGRAM_ID`, seeds, timeouts) also unused anywhere — confirmed via search. `constants.rs`'s `PLAYER_SEED = b"player"` doesn't even match the real on-chain profile seed (`b"profile"`), confirming it's stale scaffold rather than a currently-correct-but-unused alternative. The real, live instruction/seed layer in this same tree is `solana::program_interface::instructions` (heavily used — confirmed via search across lobby.rs, integration/systems.rs, rollup/bridge.rs, ui/account/profile_creation.rs), so this is a partial-tree issue, not all of `src/solana/` being dead. | Documented in place. Not added to `legacy-cleanup-audit.md` since (unlike the fully-orphaned findings) these files are compiled and exported — safe to delete but needs the same "confirm no external caller" check as the other compiled-but-unreferenced findings above. |
| **`tauri/src/`: `types/*` (whole directory), `services/auth.rs::AuthState`, and `services/config.rs`** | A substantial fraction of `tauri/src/` is dead scaffolding, all pre-marked `#![allow(dead_code)]`, confirmed unused via search: `types::auth`'s DTOs, `types::config`'s `AppConfig`/`WindowConfig`/`SecurityConfig`, `types::ipc`'s `IpcCommand`/`WindowCommand`/`AuthCommand`/`ConfigCommand`, `services::auth::AuthState` (registered via `app.manage()` but never read/written by any command), and `services::config`'s env-reading functions (`get_backend_url` etc. — `main.rs` defines its own separate, locally-scoped equivalents instead of importing these). The bridge's real state/config/commands all live directly in `main.rs` and `services::ipc::{WindowCommands, IpcCommands}`. | The `#![allow(dead_code)]` annotations already present suggest this was already known to the author as an abandoned typed layer, not a new discovery — just previously undocumented as such at this scale. Left in place and documented; safe to delete once confirmed no external caller. Given the scale, this reads as an earlier "properly typed" architecture attempt that got superseded by ad hoc enums/functions inlined in `main.rs` and `services::ipc`. |
| `src/multiplayer/rollup/mvp_plugin.rs::{EphemeralMvpState, EphemeralMvpPlugin}` | An early, minimal MagicBlock state tracker — compiled (declared in `rollup/mod.rs`) but confirmed unused anywhere else. The real ER delegation lifecycle lives in `rollup::magicblock`/`rollup::manager`. | Low-stakes — safe to delete once confirmed no external caller; not added to `legacy-cleanup-audit.md` since it's compiled-but-unreferenced rather than fully orphaned, same tier as `integration/rpc.rs` above. |
| `src/game/sync.rs::GameSyncPlugin` | Registered in `GamePlugin` (`app.add_plugins(GameSyncPlugin)` in `src/game/plugin.rs`) with a comment claiming "Add network sync plugin for P2P multiplayer" — but `GameSyncPlugin::build` is an empty no-op. Real multiplayer sync runs through `multiplayer::network`/`multiplayer::rollup` instead. | Fixed the misleading comment and documented the plugin as a registered no-op. Whether to remove the registration entirely is a small follow-up, not done here (zero behavior change either way since it does nothing). |
| `GET /api/debug/tx/{signature}` (docs) vs. actual route | Root `CLAUDE.md`'s Observability section and `backend/src/signing/routes/README.md` both documented this as `GET /api/debug/transaction/:signature`; the actual mounted route (`routes/debug.rs::debug_routes`) is `GET /api/debug/tx/{signature}`. Separately, the handler always returns `success: true` with no real logs/account-changes/fee data for any signature — `solana::debug_transaction` is an explicit stub (RPC transaction-status client types aren't wired up in this build). | Fixed the path in both docs and added accurate stub-behavior docs to the handler and the underlying stub function, ahead of the normal Phase 7 pass since it was found while auditing `signing/solana/debug.rs`. The stub itself is still unimplemented — a real fix needs the RPC transaction-status client wired in, out of scope for a doc-only pass. |

## Phase 9 — End-to-end testing

| Gap | Location | Plan | Status |
|---|---|---|---|
| Program instruction handlers lack inline unit tests | `programs/xfchess-game/src/*_ix/` (only 7 files repo-wide had `#[cfg(test)]`) | Extended the `elo/glicko2.rs` pattern to `lifecycle/guards.rs`, `elo/rating.rs`, and `governance_ix/resolution.rs` (17 new tests: phase-guard error propagation, ELO boundary/rounding, dispute-resolution winner validation and dual-record mutation). `magicblock/routing.rs` (the plan's other suggested target) is a single doc constant with no logic to test. `cargo test -p xfchess-game --lib` passing (47 tests). Remaining `*_ix/` handlers still rely on `programs/xfchess-game/tests/*.rs` integration coverage only — not exhaustively extended further this pass. | partial |
| Parked test cited nonexistent doc, drifted API (`initialize_pools` claimed removed but wasn't; `swiss_routes` really was split into `swiss_read_routes`/`swiss_admin_routes`; `tournament_player_app_state_routes` doesn't exist) | Was `backend/tests/disabled/swiss_integration_test.rs` | Rewritten as `backend/tests/swiss_tournament_e2e.rs` on the `e2e_api.rs` in-process oneshot pattern (`build_app_router`, not hand-nested routers). Seeds the tournament directly via `TournamentStore::create` instead of the on-chain-coupled `POST /admin/tournament/create`; drives join/initialize/pairings/results/standings for real over HTTP. Covers 8-player Swiss round 1 → auto-advance → round 2, unique coverage `e2e_api.rs` doesn't have. Passing (`cargo test -p backend --test swiss_tournament_e2e`). | done |
| Web e2e coverage is a single spec, not run in CI | Was `xfchessdotcom/tests/e2e/seo.spec.ts` only; `.github/workflows/ci.yml` `web-ci` job only linted+built | Added `homepage.spec.ts` (hero/feature sections, nav presence), `nav.spec.ts` (Play/logo navigation, wallet-picker modal open/close, mobile menu toggle), `play-downloads.spec.ts` (Windows/macOS/Linux release-asset redirect and API-failure fallback, both against a mocked GitHub API so it's network-independent). Added `npm run test:e2e` (`playwright test`) and wired it into `web-ci` after the build step. All 91 specs (80 existing + 11 new) passing locally. | done |
| Zero test infra | `tauri/tournament-admin`, `tauri/wallet-ui`, `tauri/viz` | Added Vitest to all three, covering the pure logic each actually has: `tournament-admin`'s `config/environments.ts` + `services/sol.ts` (18 tests), `wallet-ui`'s newly-exported `getConnectedProvider`/`resolveExistingUsername` from `App.tsx` (7 tests), `viz`'s `lib/topology.ts::buildTopologyOption` (7 tests). New `tauri-apps-test` CI job (matrix over the three). **Not done**: a full Tauri-shell smoke test (WebDriver + native binary build) — logic/hooks coverage only this pass. | partial |
| No automated cross-component e2e | Flows described in `docs/runbooks/tournament-lifecycle.md`, `game-settlement.md`, `magicblock-lifecycle-devnet.md` | Not automated (all three flows need real wallet signing and/or a live devnet ER — not achievable in-process the way `e2e_api.rs`/`swiss_tournament_e2e.rs` are). Instead verified all three against current code: tournament instruction names (`initialize_tournament`, `start_tournament`, `claim_tournament_prize`, `distribute_tournament_prizes`, `close_tournament`), `common::escrow::pot`, and the MagicBlock runbook's pinned toolchain versions (Anchor 1.1.2, Solana 3.1.12, `ephemeral-rollups-sdk` 0.16.2) all match current `Cargo.toml`/`lib.rs` exactly — no drift found, no edits needed. (This also caught two stale project-memory entries about the SDK version ceiling, corrected separately.) | done (verified, not automated — see plan column) |
