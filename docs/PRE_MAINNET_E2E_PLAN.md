# XFChess — Pre-Mainnet E2E Test Plan

**Date:** 2026-08-02
**Status:** Draft — derived from a source-level re-verification of an external
audit checklist against current `HEAD`, not from the checklist's own wording.
**Companion docs:** [THREAT_MODEL.md](THREAT_MODEL.md) (attacker/abuse model),
[AUDIT_TRACKING.md](AUDIT_TRACKING.md) (doc/test coverage by module),
[PRODUCTION_REALITY_PLAN.md](PRODUCTION_REALITY_PLAN.md) (ops hardening),
[legacy-cleanup-audit.md](legacy-cleanup-audit.md) (dead code inventory).

## Why this doc exists, and how it was built

A pre-mainnet audit checklist was supplied covering eight areas: money seams,
keys/authority, anticheat, compliance, dependency risk, code ownership, test
coverage, and onboarding. **Compliance/KYC coverage is deferred — removed
from this pass at the user's request (2026-08-02); the source checklist's §4
and the KYC-gating half of §2 aren't in this doc.** Rather than transcribe
the rest, every load-bearing claim was **re-verified against current
source** (four independent deep reads across the money-critical paths, the
authority/anticheat surface, and the dependency/test-coverage inventory — all
with file:line citations).
Several of the checklist's factual premises turned out to be **stale or
wrong** against current code; those are corrected inline, not just repeated.
The result is this doc: not the checklist restated, but the checklist turned
into concrete, gradeable end-to-end test scenarios, each anchored to what the
code actually does today.

**Status legend** (per item, distinct from the original's priority legend):
- ✅ **Solid** — verified correct, has real test coverage.
- ⚠️ **Real, works, undertested** — the mechanism is correct but has a
  verified gap (missing negative test, a code path that bypasses it, a CI
  test that silently no-ops).
- ❌ **Gap** — verified absent or verified broken.

Priority legend (kept from the source checklist): 🔴 hard gate · 🟡 before
scale · 🟢 hygiene.

---

## 1. The money seams — the only bugs that cost real funds 🔴

### 1.1 `finalize_game` fee_payer constraint — ✅ solid, ⚠️ untested negative case

**Verified state:** the constraint is live, `programs/xfchess-game/src/game_ix/finalize.rs:14-20,39-40`:
```rust
#[account(mut, close = fee_payer, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
pub game: Account<'info, Game>,
...
#[account(mut, constraint = fee_payer.key() == game.fee_payer @ GameErrorCode::FeePayerMismatch)]
pub fee_payer: SystemAccount<'info>,
```
`FeePayerMismatch` is defined at `errors.rs:227-228` and is also enforced at
three other sites (`game_ix/record.rs:13`, `lifecycle/transitions.rs:23`,
`delegation_ix/delegate.rs:47`) — this is a genuinely repo-wide invariant, not
a one-off check.

**Gap:** grepping every test file in `programs/xfchess-game/tests/` for
`FeePayerMismatch` returns **zero hits**. The only test that touches
`fee_payer` (`global_session_settlement_tests.rs:109-174`) always supplies
the *correct* payer. No test ever supplies a mismatched one.

**Test to add** — `programs/xfchess-game/tests/global_session_settlement_tests.rs`
or a new `finalize_fee_payer_tests.rs`:
- `finalize_game_rejects_mismatched_fee_payer` — build and submit a
  `finalize_game` instruction substituting a funded-but-wrong `SystemAccount`
  for `fee_payer`. Assert the transaction fails with `FeePayerMismatch`
  (check the Anchor custom-error code, not just "it failed").
- While in there: assert the *correct*-payer path still refunds rent to
  exactly `fee_payer`'s balance delta (regression guard against a future
  change to the `close =` target).

---

### 1.2 Dual-transport dedup under reordering — ⚠️ real, works, but asymmetric across transports and undertested at the integration level

**Verified state:** dedup is **content-addressed**, not nonce-based — both
transports check/insert `version_hash(fen_after, move_number)` into
`CausalChainState.applied_versions: HashMap<game_id, HashSet<String>>`
(`src/multiplayer/types.rs:99-112`); first arrival wins, second is dropped:
- Gossip path: `src/multiplayer/systems.rs:696-713`.
- Relay/Braid path: `src/multiplayer/network/braid_transport.rs:361-369`.

The `seq` field the checklist called out (*"to detect replays and sequence
gaps independently of nonce"*, `protocol.rs:53-56`) **is real and enforced —
but only on the gossip path** (`systems.rs:610,644-693`: gap detection +
equivocation/roster checks). **Braid-relay-reconstructed moves always set
`agent_id: Vec::new(), seq: 0`** (`braid_transport.rs:375-379`), which
`systems.rs`'s guard (`if !agent_id.is_empty() && *seq > 0`) explicitly skips.
So: the relay transport has **no seq-gap or equivocation protection at all**
— it relies entirely on `applied_versions` content-hash dedup. That's not
necessarily unsafe (a duplicate/replayed move produces the same hash and gets
dropped either way), but it means the two transports are **not symmetric**,
and nothing currently tests that asymmetry is actually safe rather than
accidentally safe.

A separate `NonceSequencer` (`src/multiplayer/network/reorder.rs`) buffers
out-of-order arrivals by the legacy `nonce` field and is swept for staleness
(`systems.rs:835-871`).

**Existing test coverage:** a hand-rolled deterministic-xorshift fuzzer
(`reorder.rs:219-306`, 500 trials × 40 moves) feeds interleaved/
duplicated/single-transport-dropped sequences into `NonceSequencer` **in
isolation** and asserts strictly-increasing, gap-free, no-fork release order.
It does **not** exercise the real `applied_versions`/seq-gap/equivocation
pipeline in `systems.rs`, and does not exercise the relay path's bypass of
that pipeline. No `proptest`/`quickcheck`/`arbitrary`-driven fuzzing exists
anywhere in the workspace (confirmed by repo-wide grep) — `reorder.rs:194`'s
own comment says the hand-rolled PRNG was a deliberate choice to avoid adding
a fuzzing dependency.

**Test to add** (this is the highest-value single test in this whole plan —
it is the seam the audit checklist correctly flagged as the most likely
source of a lost-funds bug):
- A new integration-level property test — either extend `reorder.rs`'s
  harness to drive the *actual* `systems.rs`/`braid_transport.rs` handlers
  against a `CausalChainState`, or add one in `src/multiplayer/network/` —
  that feeds an interleaved, duplicated, dropped, and reordered stream of
  moves **across both transports simultaneously** (some delivered only via
  gossip, some only via relay, some via both) and asserts:
  1. Final board state is always the single correct lineage (no fork).
  2. No move is ever applied twice regardless of which transport(s)
     delivered it.
  3. A move delivered *only* over the relay path (agent_id empty, seq 0)
     cannot be used to smuggle in a different `version_hash` than what the
     gossip-verified sender actually produced — i.e., prove the content-hash
     dedup alone is sufficient in the seq-free case, don't just assume it.
- A narrower, cheaper unit test: two relay-delivered `NetworkMessage::Move`s
  with identical `version_hash` from different claimed origins — assert the
  second is silently dropped, not double-applied.

---

### 1.3 Clock authority → timeout → payout — ✅ solid, ⚠️ two untested branches

**Verified state:** this seam is **not** gameable the way the checklist
worried it might be. There is no `ClockState` — the actual type is
`NetworkMessage::Clock{white_ms, black_ms, timestamp_ms}`, and "opponent
flagged" is a **purely local, client-side** UI event
(`FlagTimeoutEvent`/`GameOverState`, `src/game/systems/game_logic.rs:178-241`)
that gets gossiped to the peer only for UI purposes
(`systems.rs:1204-1209`). It has **zero on-chain effect**.

The real, money-relevant path is `game_ix/timeout.rs:12-25`
(`ClaimTimeout`, permissionless — any `Signer` can call it) →
`lifecycle/terminal.rs:35-58` (`finish_by_timeout`):
```rust
let inactivity_window = clock::inactivity_window_seconds(game); // 3x base_time, or 24h untimed
require!(now - game.updated_at > inactivity_window, GameErrorCode::TimeoutNotExpired);
let white_timed_out = game.turn % 2 == 1;
let winner = if white_timed_out { game.black } else { game.white };
```
`now` is `Clock::get()?.unix_timestamp` — the Solana runtime clock, not a
caller-supplied value. Who timed out is derived entirely from on-chain
`game.updated_at`/`game.turn`. The caller supplies no data the payout
decision depends on, so no client-side clock message — spoofed or not — can
influence money. **This closes the checklist's §1 clock-authority concern.**

**Gap:** only one existing test exercises this
(`er_delegation_tests.rs:198-224`, `claim_timeout_mutates_only_game_even_when_delegated`)
— it covers exactly one branch (white timed out → black wins, on a
delegated game). Untested: the `TimeoutNotExpired` rejection (calling before
the window elapses), and the opposite branch (black timed out → white wins).

**Test to add** — `programs/xfchess-game/tests/`:
- `claim_timeout_rejects_before_inactivity_window_elapses` — call
  `ClaimTimeout` immediately after a move, assert `TimeoutNotExpired`.
- `claim_timeout_awards_white_when_black_flags` — the mirror of the existing
  test (`game.turn` even → white wins), to lock in both parities of the
  `white_timed_out` branch.
- One test on the crank twin, `finish_by_timeout_if_expired`
  (`terminal.rs:64-86`, used by `crank_ix::crank_time_check`) — confirm it
  reaches the same outcome as the manual path, since two independent code
  paths compute the same payout decision and only one is currently tested.

---

### 1.4 `version_hash` collision for non-move events — ❌ real bug, but on dead code (verify it stays dead)

**Verified state:** the checklist's premise about 64-bit truncation is
**stale** — `version_hash` was widened to a full 32-byte SHA-256 digest
(`braid_chess/src/patch.rs:82-86`) specifically because it's now the durable
dispute-evidence key, not just a P2P equivocation check. The collision
concern is otherwise real: `BraidPatch::from_message` (`patch.rs:46-58`)
seeds non-move events with a constant `version_hash(version_seed, 0)`
(line 51), so two different non-move events sharing a `version_seed` do
collide as described.

**But:** `from_message` has **zero call sites anywhere in the repo**
(confirmed by grep) — live resign/draw/offer events go through
`NetworkMessage`/`ChessMessage` (P2P) or `backend/src/signing/routes/game_log.rs`'s
SQLite log directly, both bypassing this function. This was already
identified and consciously left as dead code in
`docs/plans/networking-hardening-plan.md:87-93,270-273`. No uniqueness
constraint exists on `version_hash` anywhere (`game_event_log`'s primary key
is `(game_id, seq)`, not the hash) — the only integrity check is
parent-linkage (`game_log.rs:257-273`, `PutEventError::ParentMismatch`).

**Action, not a test:** this is a "don't let it wake up broken" item, not an
active seam. Either (a) delete `from_message` in the next cleanup pass since
it has no callers, or (b) if it's being kept for a planned future caller, fix
the seed (fold in a monotonic counter or `agent_id`) *before* wiring it up,
and only then add a collision-regression test. Do not add test coverage to
dead code — track it in `docs/legacy-cleanup-audit.md` instead so it doesn't
silently get resurrected uncollision-fixed.

---

### 1.5 `recover_stuck_delegation` — ✅ guard is correct, ❌ zero audit trail, ⚠️ CI silently skips half its tests

**Verified state:** the guard is exactly as tight as it should be
(`governance_ix/recover_stuck_delegation.rs:59-68`): requires
`game.owner == crate::ID` **and** `game.data_is_empty()` — the precise
post-`force_undelegate_after_timeout` wiped state, erroring
`GameNotStuckDelegation` otherwise. It never touches any ELO/profile account
(not even present in the `Accounts` struct). The signer is constrained to
`dispute_authority` (`constants.rs`), matching the same key used for dispute
resolution.

**Gap 1 — no audit trail at all.** Grepping the entire file for `emit!`
returns nothing. There is no on-chain event, and therefore no way to later
answer "who attested white/black for this recovery and when" from chain
data alone — exactly the accountability trail the checklist asked for,
confirmed missing. This is already flagged as an open backlog item in
`docs/plans/networking-hardening-plan.md:306-315` (P5).

**Gap 2 — CI coverage is weaker than the test file count suggests.**
`programs/xfchess-game/tests/er_recovery_tests.rs` has 6 tests covering the
escrow split, the non-stuck rejection, and the wrong-authority rejection —
but **2 of the 6 silently `return` (skip) unless `keys/dispute_authority.json`
exists on disk**, and that file is gitignored/untracked
(`git ls-files keys/` shows only `KEYS_README.md`). On a fresh clone or in
CI without that file provisioned, those two tests pass by doing nothing.

**Action + tests to add:**
- **Code change, not just a test:** add an `emit!` event
  (e.g. `StuckDelegationRecovered { game_id, dispute_authority, white_authority, black_authority, white_share, black_share, timestamp }`)
  to the handler, then a test asserting the event fires with correct fields
  on a successful recovery.
- **CI fix (small, high value):** either commit a devnet-only
  `dispute_authority` test keypair (safe if it's a dedicated devnet-only key
  holding no mainnet authority and no funds) so `keys/dispute_authority.json`
  exists in CI, or provision it via a CI secret before the test job runs.
  Until then, change the silent `return` into an explicit `#[ignore = "needs keys/dispute_authority.json"]`
  or a loud `eprintln!("SKIPPED: ...")` so a `cargo test` run visibly shows 2
  tests were not exercised, instead of reporting a clean pass.

---

### 1.6 MagicBlock ER-unreachable dead-ends — ✅ solid, wired, idempotent

**Verified state:** fully wired, not dead. `settlement_worker.rs`'s
stale-delegation branch (lines 277-317) fires
`request_force_undelegate_for_stale_game` once after
`STALE_DELEGATION_SECS` (explicitly fire-once per its own comment), then
`force_undelegate_if_request_expired` (line 617) polls the on-chain
`UndelegationRequest` PDA's `expires_at_slot` (~60min) and submits
`force_undelegate_after_timeout` once elapsed, immediately followed by
`auto_recover_stuck_delegation` (line 694) to release escrow. Idempotency is
explicit in-code: *"Calling this again once a request already exists is a
harmless on-chain no-op"* (line 545-546), with a fallback log+metric path if
escrow was already drained by a concurrent manual call (lines 719-725).
`force_recovery.rs`'s module doc confirms the escape hatch needs no
cooperation from the ER validator at all — the base layer alone can force
the outcome. No dead end was found for any stale-delegated wagered game.

**Test to add / rehearsal to schedule** (not a code gap, an ops-rehearsal
gap): `crates/solana/er-cu-benchmark/src/recovery_drill.rs` already contains
`run_stuck_delegation_drill` (a real ~60-70 minute live-devnet exercise of
the full `request_force_undelegate → force_undelegate_after_timeout →
recover_stuck_delegation` chain) and `run_crank_liveness_drill` (~1-2 min).
Neither is automated in CI (they're opt-in, live-devnet, real-time-bound —
correctly so). **Action:** schedule `run_stuck_delegation_drill` as a
recurring pre-mainnet rehearsal (e.g. monthly, or before any change to
`delegation_ix`/`settlement_worker.rs`), and log results in
`docs/runbooks/magicblock-lifecycle-devnet.md` rather than running it ad hoc
and discarding the output.

---

### 1.7 Nonce replay across reconnect/resync — ⚠️ real gap on the spectator path, ✅ safe on the live-player path

**Verified state:** two independent resync mechanisms with different
guarantees.
- **Live 1v1 players** (`NetworkMessage::ResyncRequest/Response`,
  `systems.rs:1151-1169`): triggered by `NonceSequencer::expire()`
  (`reorder.rs:94-109`). This is a full authoritative-state stomp (overwrite
  local FEN), and `expire()` snaps the sequencer's `expected` counter forward
  first — post-resync moves resume clean nonce tracking. **No double-apply
  risk.**
- **Spectator Braid catch-up** (`BraidResyncRequest/Response`,
  `src/multiplayer/rollup/bridge.rs:425-498`, sole sender
  `spectator.rs:219`): these `MovePayload`s carry **no nonce/seq field at
  all** (confirmed by `CausalChainState.applied_versions`'s own doc comment,
  `types.rs:104-106`), and `apply_braid_resync_to_spectator`
  (`spectator.rs:299-339`) feeds them straight to
  `handle_network_moves` (`network_move.rs:15-176`) **without touching
  `applied_versions` or `NonceSequencer` at all**. In practice this doesn't
  corrupt the board today — `handle_network_moves` rejects a replay because
  the source square is already empty — but that's an *incidental* legality
  rejection standing in for an *explicit* dedup invariant. There's also a
  live no-op bug adjacent to this: `spectator.rs:335`
  (`let _ = session.applied_move_count; // acknowledged; VPS poll will deduplicate`)
  reads the counter but never advances it, so the same moves can be
  re-fetched and re-queued every poll cycle (redundant traffic/log spam, not
  corruption).

**Test to add:**
- `spectator_resync_does_not_reapply_already_seen_moves` — feed a spectator
  session a resync response containing moves it has already applied, under
  adversarial reordering, and assert (a) no duplicate `NetworkMoveEvent` is
  emitted, (b) board state is unchanged, not just "didn't crash."
- Fix `applied_move_count` to actually advance on acknowledged moves (small
  bug, currently dead-write) and add a regression test that the VPS poll
  dedup counter increases monotonically with real progress instead of
  staying flat.
- Once the above lands, consider whether spectator catch-up should route
  through the same `applied_versions` content-hash check the live paths use,
  rather than relying on move-legality-as-dedup — this is the more durable
  fix and turns an incidental safety property into an explicit one.

---

## 2. Keys & authority — the non-custodial promise 🔴

### 2.1 The five authorities — ✅ confirmed, separations intact

`programs/xfchess-game/src/constants.rs` defines all five as documented:

| Authority | Purpose | Pubkey |
|---|---|---|
| `kyc_authority` | verifies profiles | `2mh7zXgZHaeDnroJQQdHnLNiierWXdn43VnATbGdATZK` |
| `dispute_authority` | resolves disputes + `recover_stuck_delegation` | `HAHgvXf6uYxTqEuUnkkzTS1EQD8sYd342zgxM2wdqpa2` |
| `link_authority` | external-ELO linking | `42fiB5KcC1jEVXxmgPoWqpA3zuKEsZGu77YHmCwNEcrh` |
| `vps_authority` | backend operational signer | `HZTwvN9AUK1n9jmQydrh5vkpdCBZm13W7qD9jtPZJSQc` |
| `treasury_authority` | treasury withdrawal | `9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd` |

The rotate-before-mainnet TODO is still present, `constants.rs:42-43`. The
deliberate separations are both documented in-code and verified: `vps_authority`
is explicitly *not* the program upgrade authority (`constants.rs:85-90`), and
`treasury_authority` is explicitly separate from `vps_authority`
(`constants.rs:102-105`). A test (`constants.rs:169-181`,
`production_authorities_are_not_default_pubkeys`) checks non-default values
but not pairwise distinctness — **test to add:** assert all five constants
are pairwise distinct, so a future edit can't accidentally collapse two
authorities onto the same key without a test failing.

**Still open (unchanged from prior audit tracking, not re-litigated here):**
multisig (e.g. Squads) for `treasury_authority` and `dispute_authority` before
real money — no multisig infrastructure was found anywhere in the repo.
Secret rotation itself is still pending per [project memory](../CLAUDE.md)
(local devnet keys were regenerated; the Helius dashboard rotation and
git-history exposure remain open).

### 2.2 Platform fee has no on-chain bound — ⚠️ real gap (correcting the source checklist)

**Country fees are NOT hardcoded to zero** (correcting the source checklist,
which assumed they were pending counsel): `game_ix/common.rs:51-55` sets
`game.country_fee` from a live, client-supplied `platform_fee` argument,
computed backend-side from a real SOL/GBP rate (`routes/rates.rs:465-474`).
The `country_fee: 0` seen in `global_create.rs:173`/`session_create_game.rs:235`
is a placeholder struct literal immediately overwritten by
`init_game_fields(...)` a few lines later — not a live zero-fee bug.

**A different, real gap exists here instead:** `platform_fee` is
client-supplied with **no on-chain bound** on its magnitude — nothing was
found constraining it to a sane range.

**Test to add:**
- `create_game_rejects_unreasonable_platform_fee` (or confirm a bound
  already exists elsewhere and this is a non-issue) — add an explicit
  on-chain or backend-side cap on `platform_fee` if none exists, plus a test
  proving an absurd value is rejected rather than silently accepted.

### 2.3 Network identity ↔ wallet coherence — ⚠️ real gap, but scoped to P2P game-record integrity, not funds

**Verified state:** `agent_id` on a move is an **ephemeral per-session
gossip-signing key** (`Keypair::new()`,
`solana/integration/systems.rs:358`) — not the wallet key and not the Iroh
node id. `bind_identity` (`systems.rs:246-261`) correctly overwrites a
claimed `agent_id` with the cryptographically-verified P2P envelope signer,
and a roster restricts accepted movers to previously-announced signing keys.

**The gap:** the binding from that ephemeral signing key to a real wallet
(`SessionInfo{player_pubkey, session_pubkey, signing_pubkey}`,
`protocol.rs:62-86`) is **self-asserted by the claiming peer, with no wallet
signature over the ephemeral key** (`solana/integration/systems.rs:491-497`).
A peer can announce any `player_pubkey` alongside its own freshly-generated
`signing_pubkey`, and subsequent roster/`bind_identity` checks only verify
"this message came from the key that announced itself" — not "this key's
owner controls that wallet." **Funds are not exposed** by this — the
authoritative wallet-authority check for money/results happens on-chain,
independent of this gossip layer. But move *attribution* in the P2P
record (and anything downstream that trusts it, e.g. spectator UI or
future dispute-evidence tooling built on the Braid event log) is spoofable.

**Test to add:**
- `spoofed_session_info_cannot_move_funds` — deliberately construct a
  `SessionInfo` claiming a victim's `player_pubkey` with an attacker-controlled
  `signing_pubkey`, drive moves through the gossip layer under that identity,
  and assert (a) it's accepted at the gossip layer as the checklist worries
  (documenting the real gap), but (b) prove no on-chain instruction can be
  reached this way that would move escrow funds or affect `finalize_game`'s
  payout — i.e. turn the informal "money layer is separate" claim into an
  explicit, regression-tested boundary rather than an assumption.
- If move-attribution ever becomes dispute-evidence (per `version_hash`
  widening's stated purpose in 1.4), revisit whether `SessionInfo` needs an
  actual wallet signature before that happens — flag as a design
  prerequisite, not an immediate fix.

---

## 3. Anticheat — the #1 revenue risk 🔴

### 3.1 Detection coverage — ✅ real, and now tested (audit-tracking is stale here)

`crates/shared/xfchess-anticheat` runs genuine detection: Stockfish-based CPL
scoring (`lib.rs:118-172`), top-1-move-rate/complexity
(`build_side_analysis`, `lib.rs:174-214`), think-time/blur/timing anomalies
(`features/{timing,blur,complexity}.rs`), an ELO-calibrated scorer
(`scoring/mod.rs`, thresholds `config.rs:67-68`: review=0.60, flag=0.80), and
a 30-game rolling per-player baseline (`cross_game/mod.rs:14-60`,
advisory-only). **11 files now carry `#[cfg(test)]` modules**, including a
regression guard for the CPL-arithmetic bug the most recent commit fixed —
`docs/AUDIT_TRACKING.md`'s "no test files found for this crate" line is
stale and should be updated in the next audit pass.

### 3.2 Settlement coupling — ❌ headline finding: 1v1 wagered payouts happen before analysis, with no clawback path

This is the single most important finding in this whole plan, and it
directly matches the checklist's framing of anticheat as the #1 revenue
risk — just sharper than the checklist could see without reading the worker.

For **ordinary 1v1 wagered games**, `settlement_worker.rs` submits the
on-chain `finalize_game` payout **first** (lines 830-834 log the
already-broadcast signature) and only **afterward** enqueues anti-cheat
analysis (lines 862-877, `anticheat_enqueue.rs:37`). **A cheat verdict
cannot gate or claw back a 1v1 wagered payout — it is purely post-hoc and
advisory for the majority of games on the platform.**

**Tournament prizes are the one place with real gating**
(`tournament_scheduler.rs:498-576`, `PrizeGate`/`anticheat_gate`): holds
distribution for up to `PRIZE_HOLD_WINDOW_SECS` (15 min) while analysis is
pending, then excludes any wallet with a `Flag` verdict. Past that window,
**pending-but-unanalyzed games are paid anyway** (line 494: *"analysis lag
must not freeze payouts indefinitely"*) — only already-known flags are
withheld.

**No appeal/reversal path exists anywhere.** The withholding comment says
*"resolve via governance dispute"* (`tournament_scheduler.rs:588,658`), but
`governance_ix/{dispute,resolve,resolution,claim_stale_dispute}.rs` contain
**zero references** to anticheat/verdict/flag — no on-chain instruction is
aware of anti-cheat state at all. `FlaggedGameRepository`
(`admin.rs:206-208,521-530`) is a separate manual-flag mechanism, not a
verdict-reversal path. A `Flag` is effectively final for tournament prizes
(the place stays undistributed forever, no coded route to release it later)
and has zero effect at all on 1v1 wagers.

**This needs a product decision before broad/paid rollout, not just a
test:**
- Option A: give 1v1 wagers the same short hold-window pattern tournaments
  already have (delay `finalize_game` submission behind a bounded analysis
  window).
- Option B: keep 1v1 post-hoc-only as a deliberate choice, but then build a
  real clawback/dispute path for late-arriving Flags (currently none exists
  — a Flag today just sits in a report table with no on-chain consequence).
- Either way, build the false-positive **appeal/reversal path** the
  checklist asks for — today a Flag can never be un-flagged or released.

**Tests to add once a decision is made:**
- `tournament_prize_gate_pays_unanalyzed_game_after_hold_window_expires` —
  a test that exercises the exact documented race (start distribution right
  at the `PRIZE_HOLD_WINDOW_SECS` boundary with analysis still pending),
  confirming the current documented-but-untested behavior.
- `flagged_verdict_after_1v1_payout_has_no_effect` — a test making the
  current gap explicit (submit a Flag verdict after `finalize_game` has
  already paid out; assert nothing on-chain changes) — this should be the
  regression test that starts failing once a real clawback path is built,
  proving the fix landed.
- Whatever appeal mechanism is chosen needs its own acceptance test
  (`appealed_flag_can_be_reversed_before_hold_window_expires` or similar).

### 3.3 Clock-manipulation as a cheat vector — ✅ closed, see §1.3

Already covered under the money-seams section: client-asserted clock state
has zero on-chain effect, so this isn't a distinct anticheat surface beyond
what §1.3 already tests.

### 3.4 P2P evasion resistance — not independently re-verified this pass

The checklist's concern ("can a cheating client just not report the
signal") wasn't re-verified against the anticheat ingest path in this pass —
worth a follow-up read of `backend/src/signing/anticheat_enqueue.rs` and
`tasks/anticheat_worker.rs` specifically for what happens if a client omits
telemetry (blur/timing) entirely versus sending it honestly, since the P2P
architecture means the backend can't compel a client to report.

---

## 4. Dependency & vendored-fork liability 🟡

### 4.1 `iroh-gossip` fork provenance — ❌ confirmed undocumented, un-diffable

`crates/zarathustra_net/iroh-gossip/` is 17 files, **8,516 lines**. Its
`README.md` says only "FORK of the iroh-gossip crate from n0-computer/iroh...
may have custom patches" — no source tag/commit named, no changed-files
list. `CHANGELOG.md` is upstream's own auto-generated history (git-cliff),
not a local diff. No `FORK_NOTES`/`PATCH`/`VENDOR` file exists anywhere. Git
history doesn't help either — the crate arrived already-vendored in a
directory-rename commit (`67a1fbe85`), so there's no pinned upstream commit
to diff against even via `git log`. **This is, today, a fork nobody can
safely diff against upstream — confirmed, not just suspected.**

**Action (not a test):** write `crates/zarathustra_net/iroh-gossip/FORK_NOTES.md`
documenting (a) the upstream commit/tag this was forked from (even an
approximate one, cross-referenced against `CHANGELOG.md`'s last upstream
entry — v0.101.0, "Update to iroh 1.0" — is better than nothing), and (b)
every local change, even if the answer turns out to be "none yet found."
Do this before the next Iroh-adjacent change, while it's still possible to
reconstruct.

### 4.2 Iroh version pin is exact at the workspace root but not inside the fork itself — ⚠️ works today, not structurally enforced

Root `Cargo.toml` exact-pins `iroh = "=1.0.3"` / `iroh-base = "=1.0.3"` in
`workspace.dependencies`. `braid-iroh/Cargo.toml` correctly inherits that via
`iroh = { workspace = true, ... }`. **But `iroh-gossip/Cargo.toml` does not
inherit the workspace pin** — it declares its own loose `iroh = { version = "1", ... }`
(effectively `^1`). `Cargo.lock` currently resolves everything to `1.0.3`
(no live drift today), but nothing in the fork's own manifest would stop a
future `cargo update` from moving it independently of the root pin — exactly
the "accidental Iroh bump silently breaks gossip re-sync mid-game" risk the
checklist warned about, just located one file deeper than expected.

**Fix (small, concrete):** change `iroh-gossip/Cargo.toml` to inherit
`iroh = { workspace = true, ... }` / `iroh-base = { workspace = true, ... }`
the same way `braid-iroh` already does, so the fork can't silently drift
from the pin the rest of the workspace relies on. Add a CI check (or just a
`cargo tree -i iroh` assertion in a test) that only one `iroh` version
resolves workspace-wide.

### 4.3 Pinned-version rationale — 🟢 inconsistent, fixable in one pass

Root `Cargo.toml` exact-pins `anchor-lang`/`anchor-spl` (`=1.1.2`),
`solana-program`/`solana-sdk` (`=3.0.0`), `ephemeral-rollups-sdk`
(`=0.16.2`), and `bevy` (`=0.19.0`) with **no rationale comment on any of
them**. `tauri/Cargo.toml` uses a loose `"2.11"` range, also uncommented.
Notably, the repo *does* have a demonstrated convention of commenting
constraints elsewhere in the same file — `winit = "=0.30.13"` has a comment
explaining it's pinned to match what Bevy 0.19 pulls in, and the
`bevy_fixed_node`/`sky` feature flags each explain why they're gated behind
an upstream fix that isn't released yet. **Action:** bring the five
uncommented pins up to that same standard — one line each is enough (e.g.
"Anchor 1.1.2: last version compatible with Solana 3.0.0's account model,
see MAGICBLOCK.md"). Cheap, and the exact reasoning is presumably already in
someone's head — write it down before it isn't.

---

## 5. Ownership & the 2am test — the anti-slop section 🟡

This repo is effectively solo-authored today (single committer across
recent history) — the "owner" column can't honestly be filled with distinct
names yet. What *can* be measured objectively is **surface area**, as a
proxy for "how much could plausibly still fit in one person's head":

| Crate / area | Lines (`.rs`) | Money/consensus-adjacent? |
|---|---|---|
| `src/multiplayer/` | 17,684 | **Yes** — dual-transport publish path lives here (§1.2) |
| `programs/xfchess-game/` | 14,715 | **Yes** — the escrow/settlement program itself |
| `crates/engine/nimzovich_engine/` | 9,199 | No — chess AI |
| `crates/zarathustra_net/iroh-gossip/` (vendored fork) | 8,516 | Indirect — transport underneath §1.2 |
| `crates/zarathustra_net/braid-http/` | 4,698 | Indirect |
| `crates/shared/xfchess-anticheat/` | 2,473 | **Yes** — §3 |
| `crates/zarathustra_net/braid-iroh/` | 1,436 | Indirect |
| `crates/zarathustra_net/braid_chess/` | 948 | **Yes** — §1.4 version-hash lives here |
| `crates/zarathustra_net/braid-core/` | 226 | Indirect |

**Reading this table for risk, not just size:** `src/multiplayer/` is both
the *largest* single area in the codebase and the one holding the exact seam
(§1.2) already flagged as the most likely source of a lost-funds bug — size
and risk overlap here, which is the worst combination for a single-owner
project. `iroh-gossip` is large, vendored, and (per §4.1) undiffable against
upstream — the least "ownable" 8.5k lines in the repo, since understanding
it means understanding someone else's protocol implementation, not this
project's own logic.

**Honest framing for the "2am test," given solo authorship:** the question
isn't "does someone else understand this" (nobody else does yet), it's
"if you're paged at 2am about `src/multiplayer/` or `iroh-gossip`, do you
still have the mental model, or would you be re-deriving it from the code
same as a stranger would?" That's a self-assessment only the author can make
honestly — this table exists to point at where that self-assessment matters
most before a team is brought on, not to answer it.

---

## 6. Test coverage at the seams 🟡

### 6.1 Two tests that silently assert nothing in CI — ❌ fix before trusting green CI runs

This is a distinct, higher-priority finding than "coverage is thin" — these
are tests that **look like** coverage and currently aren't:
- `programs/xfchess-game/tests/er_recovery_tests.rs` — 2 of 6 tests
  silently `return` without `keys/dispute_authority.json` (gitignored, not
  present on a fresh clone or CI runner).
- `programs/xfchess-game/tests/tournament_registration_e2e_tests.rs` — its
  **only** test's assertion path is entirely gated behind
  `keys/vps_authority.json` (same problem). **This file currently asserts
  nothing in CI.**

**Fix:** provision both devnet-only test keypairs in CI (secret injection or
a committed, clearly-labeled devnet-only fixture with zero mainnet
authority/funds), and until that's done, change the silent `return` into a
loud, visible skip (`#[ignore = "needs keys/vps_authority.json — see docs/PRE_MAINNET_E2E_PLAN.md §6.1"]`
or an `eprintln!` banner) so `cargo test` output doesn't read as "passing"
when it's actually "did nothing."

### 6.2 Money-seam coverage map (this doc's §1, consolidated)

| Seam | Status | Test gap |
|---|---|---|
| `finalize_game` fee_payer | ⚠️ | no negative test (§1.1) |
| Dual-transport dedup | ⚠️ | no full-pipeline property test; relay-path asymmetry untested (§1.2) |
| Clock/timeout → payout | ✅ | two branches untested (§1.3) |
| `recover_stuck_delegation` | ⚠️ | no audit event; 2/6 tests CI-skip (§1.5, §6.1) |
| MagicBlock ER dead-ends | ✅ | drill exists, not scheduled (§1.6) |
| Spectator resync dedup | ❌ | no explicit dedup, relies on incidental legality check (§1.7) |
| Tournament disconnects | manual-only (from prior audit, not re-verified this pass) | — |

### 6.3 Property/fuzz testing — ❌ confirmed absent workspace-wide

Repo-wide grep for `proptest`, `quickcheck`, `arbitrary::Arbitrary`,
`fuzz_target!`, and any `fuzz/` directory: zero hits for the first three
macros/harness patterns; the only `arbitrary::Arbitrary` impls are 5 trait
impls in `braid-http/src/types/*` with no fuzz harness driving them (no
`fuzz/` directory exists anywhere). `reorder.rs:194`'s own comment confirms
this was a conscious choice, not an oversight. Given §1.2 is this plan's
single highest-value test and is naturally property-shaped (arbitrary
interleavings of a bounded move stream), it's the natural first consumer of
real property-based testing in this workspace — recommend introducing
`proptest` scoped to that one seam rather than a workspace-wide adoption.

### 6.4 Devnet e2e rehearsal — unchanged from prior plan

The 16-person bounty tournament rehearsal (mentioned in project context) and
`recovery_drill.rs`'s live drills (§1.6) remain the right low-cost way to
exercise settlement, anticheat false positives, and account-state
persistence before a real Earn-scale bounty. Log every "lost-funds"-tagged
finding from that rehearsal as a mainnet blocker, same as the source
checklist specified.

---

## 7. Onboarding path for a future team 🟢

Unchanged from the source checklist — still the right reading order
(`programs/xfchess-game` → `crates/shared`+`crates/engine` → `braid_chess` →
`braid-iroh` → `src/multiplayer/network`) and the right one-paragraph mental
model (*"the Solana PDA is the source of truth for funds and lifecycle;
Iroh/Braid carries the live UX and is deliberately not authoritative over
money; MagicBlock is the settlement execution layer"*) — this matches
`docs/THREAT_MODEL.md`'s own trust-boundary diagram, so the two docs are
consistent. Not re-verified further this pass since it's a process
recommendation, not a code claim.

---

## Appendix — corrections to the source checklist

Recorded for credibility and so these don't get re-asked next audit pass:

| Checklist claim | Verdict | Actual state |
|---|---|---|
| `version_hash` is 64-bit truncated | **Stale** | Widened to full 32-byte SHA-256 (§1.4) — collision risk is real but on unreachable dead code, not live traffic |
| Country fees hardcoded to zero pending counsel | **Wrong** | Live, client-supplied `platform_fee` drives `country_fee`; the `: 0` seen in two files is an overwritten placeholder, not the live value (§2.2) — but this surfaced a *different* real gap (no on-chain bound on the fee amount) |
| `recover_stuck_delegation` needs an audit trail | **Confirmed as feared** | Zero `emit!` anywhere in the file — no on-chain audit trail exists at all (§1.5) |
| Anticheat crate is "least-tested in the workspace" | **Stale** | 11 files now carry real `#[cfg(test)]` coverage as of the most recent commit (§3.1) — but the *settlement-timing* gap (§3.2) is a sharper, previously-unstated finding in the same area |
| iroh-gossip fork delta undocumented | **Confirmed** | No provenance doc exists anywhere, git history doesn't help either (§5.1) |
| Dual-transport dedup via nonce/seq | **Partially correct** | True on the gossip path; the relay path bypasses seq entirely and relies solely on content-hash dedup — an asymmetry the checklist didn't anticipate (§1.2) |

**Sourcing methodology:** every finding above cites file:line against `HEAD`
as of 2026-08-02, gathered via four independent full-source read passes
(not grep-only) plus cross-checks against `docs/AUDIT_TRACKING.md`,
`docs/THREAT_MODEL.md`, `docs/legacy-cleanup-audit.md`, and
`docs/plans/networking-hardening-plan.md`. Where a docs cross-check
surfaced a contradiction (AUDIT_TRACKING.md's anticheat-test-coverage row),
it's flagged above rather than silently resolved — that doc should be
updated as a small follow-up, not as part of this plan.

**Note on scope:** compliance/KYC (source checklist §4, and the KYC-gating
half of its §2) was investigated during the original source-verification
pass but has been removed from this doc at the user's request (2026-08-02).
If picked back up later, re-run that verification fresh rather than trusting
this doc's now-deleted findings to still be accurate.
