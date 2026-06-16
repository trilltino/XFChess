# MagicBlock Integration Plan

Deep integration plan for adopting the MagicBlock framework ecosystem beyond the
base `ephemeral-rollups-sdk`. Scope is the Solana program (`programs/xfchess-game/`),
the backend (`backend/`), and the game/web clients.

> **Context:** XFChess already runs on Ephemeral Rollups via `ephemeral-rollups-sdk`.
> This doc covers the *adjacent* MagicBlock programs/SDKs that retire hand-rolled code
> or unlock new capability. It does **not** cover the Anchor 1.0 / Solana 3.0 migration
> (see "Toolchain boundary" below) — everything here targets the **current** stack:
> Anchor 0.31.1, solana-program 2.2.1, `ephemeral-rollups-sdk 0.13.0`.

---

## 0. Current state (as of 2026-06-15)

| Item | State |
|------|-------|
| `ephemeral-rollups-sdk` | **0.13.0** (bumped from 0.8.5; highest version compatible with Anchor 0.31 / solana-program 2.x) |
| `magic-resolver` | 0.2.0 (already a workspace dep — sibling of magic-router) |
| `magic-domain-program` | 0.2.0 (already pinned) |
| Validator selection | ✅ **Fixed** — now `validator: None` (router/delegation program assigns); hardcoded EU pubkey removed. `commit_frequency_ms` bug fixed (`ER_COMMIT_FREQUENCY_MS = 30s`). Magic accounts now `address`-constrained. See [delegate.rs](../programs/xfchess-game/src/delegation_ix/delegate.rs) |
| Crank / scheduled tasks | **Hand-rolled** bincode `MagicBlockInstruction::ScheduleTask` against `magicblock-magic-program-api =0.3.1` — see [schedule_time_check.rs](../programs/xfchess-game/src/crank_ix/schedule_time_check.rs). **Deferred (B1):** swap to SDK `ScheduleCrankCpi` changes the wire format → needs devnet runtime test before shipping. |
| Session keys | **Custom** (`SessionDelegation`, `GlobalSessionDelegation`, tournament sessions) — not MagicBlock `session-keys` |
| SPL (USDC) on ER | **Not used** — USDC wagers/prizes settle on base layer only |
| Delegation-status tracking (backend) | Ad-hoc (`game.is_delegated` flag) |
| Provably-fair color/pairing | **Off-chain** (trusted) |

### Toolchain boundary (why we stay on 0.13.0)

- `ephemeral-rollups-sdk` **0.14.0+** hard-requires `solana-program 3.0` (non-optional), which forces **Anchor 1.0**.
- Anchor 0.31.1 caps at `solana-program ^2` → 0.14+ is unresolvable without a full workspace migration.
- **0.13.0** is the last release on `solana-program >=1.16,<3`. It already ships the modern surface
  (`cpi`, `crank`, `ephem` intent builders, `spl`, `access_control`) — so most items below are reachable today.
- A few helpers (`delegate_account_with_any_validator`) are **0.14+ only** and are called out where they matter.

> Each framework below notes whether it needs **on-chain** changes (program redeploy),
> **client/backend** changes only, or both.

---

## 1. Priority matrix

| # | Framework | Retires / unlocks | Surface | Effort | Priority |
|---|-----------|-------------------|---------|--------|----------|
| A | **magic-router** (+ `magic-router-sdk`) | Hardcoded validator pubkey; manual base-vs-ER RPC routing | Client + backend | M | **P1** |
| B | **hydra** | Hand-rolled `ScheduleTask` crank (most version-fragile code) | On-chain + infra | M | **P1** |
| C | **session-keys** | Custom session delegation maintenance burden | On-chain + client | L | P3 |
| D | **ephemeral-spl-token** / `ephemeral-rollups-spl` | Base-layer-only USDC; enables USDC settled on ER | On-chain + client | L | P3 |
| E | **magicblock-sync** | Ad-hoc `is_delegated` tracking in backend | Backend only | S | P2 |
| F | **solana-vrf** (+ ephemeral-vrf) | Off-chain/trusted color & pairing | On-chain + client | M | P2 |
| G | **sol-chess** | (reference only) differential oracle for `chess-logic-on-chain` | Test harness | S | P2 |

Effort: S = <1 day, M = 1–3 days, L = 1–2 weeks.
Recommended sequencing in §9.

---

## 2. (A) magic-router — auto-routing & validator selection  ·  **P1**

**Repos:** `magicblock-labs/magic-router` (Rust JSON-RPC router), `magic-router-sdk` (TS).
Sibling of `magic-resolver`, which we already depend on.

### Why
The biggest correctness liability in our ER code is the **hardcoded EU devnet validator**
in [delegate.rs:26](../programs/xfchess-game/src/delegation_ix/delegate.rs#L26):

```rust
let eu_validator = "MEUGGrYPxKk17hCr7wpT6s8dtNokZj5U2L57vjYMS8e".parse::<Pubkey>()?;
let config = DelegateConfig { commit_frequency_ms: ..., validator: Some(eu_validator) };
```

This pubkey is **devnet-specific** → delegation will fail on mainnet, and it pins every game
to one region. magic-router resolves the correct endpoint/validator dynamically and routes each
transaction to the base layer or the ER automatically, so the client stops caring which validator
holds a delegated account.

### Integration steps
1. **On-chain (program):** stop pinning a validator. Set `DelegateConfig.validator = None`
   so the delegation program/router assigns one. (`delegate_account_with_any_validator()` would be
   cleaner but is 0.14+ only — on 0.13.0, `validator: None` is the equivalent.)
   - Also fix the unrelated bug on the same line: `commit_frequency_ms` is computed as
     `(valid_until as u32) * 1000` where `valid_until` is a unix timestamp — replace with a real
     interval constant (e.g. `30_000`).
2. **Client (`src/solana/`) + backend (`backend/src/signing/`):** route RPC through the
   magic-router endpoint instead of a fixed base/ER RPC URL. Replace direct `RpcClient::new(url)`
   construction in the Solana client paths; reuse the existing `magic-resolver` config to discover
   the router endpoint.
3. **Web (`web-solana/`):** swap the connection/provider to use `magic-router-sdk` so wallet txs
   are routed identically.

### Effort / risk
M. On-chain change is tiny (one struct field + one redeploy) but **must be tested on devnet**:
delegating with `validator: None` changes which validator commits state. Client routing change is
mechanical but touches every tx-submission path — do it behind the existing `--features solana` gate.

### Acceptance
- A game delegated with `validator: None` records moves on the ER and undelegates cleanly on devnet.
- No hardcoded validator pubkey remains in the program (grep clean).
- Mainnet dry-run delegation succeeds (previously impossible).

---

## 3. (B) hydra — permissionless crank scheduler  ·  **P1**

**Repo:** `magicblock-labs/hydra` — "Permissionless Solana crank for scheduling instructions."

### Why
Our time-control crank ([schedule_time_check.rs:47](../programs/xfchess-game/src/crank_ix/schedule_time_check.rs#L47))
hand-serializes `MagicBlockInstruction::ScheduleTask` with `bincode` against
`magicblock-magic-program-api =0.3.1` — **7 minor versions** behind what the SDK now uses
(0.8.8 is in our tree transitively after the 0.13.0 bump). Hand-rolling an old instruction enum
is the single most likely thing to **silently break on a validator upgrade**: if the on-chain magic
program changes the `ScheduleTask` layout, our bincode bytes decode wrong with no compile error.

### Two options
- **B1 (low effort, stay on magic program):** keep the magic-program crank but use the SDK's typed
  `ephemeral_rollups_sdk::crank::ScheduleCrankCpi` / `CancelCrankCpi` helpers (present in 0.13.0)
  instead of hand-bincode. Removes the version-skew footgun, keeps current architecture.
- **B2 (full hydra):** move scheduling to the hydra program. Permissionless cranking, no reliance on
  the magic program's scheduler, survives validator changes. More moving parts (deploy/point at hydra).

### Integration steps (B1, recommended first)
1. Drop the direct `magicblock-magic-program-api = "=0.3.1"` pin in
   [programs/xfchess-game/Cargo.toml:29](../programs/xfchess-game/Cargo.toml#L29); use the SDK's
   re-exported args (`ephemeral_rollups_sdk::...ScheduleTaskArgs`) so there's **one** version.
2. Replace the hand-built `schedule_ix` + `invoke_signed` in `schedule_time_check_crank` with
   `ScheduleCrankCpi { ... }.invoke_signed(...)`.
3. Re-test the auto-flag-timeout flow on the ER.

### Effort / risk
M. Mostly a rewrite of one instruction handler. Risk is behavioural parity of the rescheduled task
(interval, iterations) — verify a game actually auto-flags on time on the ER after the change.

---

## 4. (E) magicblock-sync — delegation-status tracking (backend)  ·  **P2**

**Repo:** `magicblock-labs/magicblock-sync` — "Real time magicblock synchronization library for
account delegation status."

### Why
The backend currently infers whether a game is on the ER from the program's `game.is_delegated`
bool, which it only learns by polling/RPC. magicblock-sync gives the backend a **real-time stream**
of delegation/undelegation events, so the relay and settlement tasks
(`backend/src/tasks/`) know exactly when a game is live on the ER vs settled on base.

### Integration steps
1. Add `magicblock-sync` to `backend/Cargo.toml`.
2. In `backend/src/signing/` (or a new `backend/src/tasks/delegation_watch.rs`), subscribe to
   delegation-status for active game PDAs; update in-memory relay state on transitions.
3. Gate auto-settlement (`tasks/`) on the synced "undelegated" signal instead of polling
   `is_delegated`.

### Effort / risk
S. Backend-only, additive, no on-chain change. Risk is low — it's an observability/coordination
improvement. Make the subscription best-effort with fallback to current polling.

---

## 5. (F) solana-vrf — provably-fair color & pairing  ·  **P2**

**Repos:** `magicblock-labs/solana-vrf`, plus `ephemeral-vrf-sdk` (ER-native VRF).

### Why
Color assignment and Swiss/bracket pairing seeds are currently chosen **off-chain** and trusted.
For a wagered, rated platform this is a fairness/dispute surface. On-chain VRF makes color
assignment and pairing seeds verifiable.

### Integration steps
1. **Color on game creation:** in `game_ix/create.rs` / `global_create.rs`, request a VRF value and
   derive `white`/`black` from it instead of caller-chosen sides.
2. **Tournament pairing:** in `tournament_ix/` (Swiss/bracket seeding), seed the pairing RNG from a
   VRF value committed on-chain so pairings are auditable.
3. Client: surface the VRF proof in game/tournament detail views.

### Effort / risk
M. On-chain VRF callback flow adds an async step to game creation (request → fulfill). On the **current
toolchain**, prefer base-layer `solana-vrf`; `ephemeral-vrf-sdk` aligns with the 0.14+/Anchor-1.0 line,
so treat ER-native VRF as part of the future migration, not this pass.

---

## 6. (G) sol-chess — differential oracle for move validation  ·  **P2**

**Repo:** `magicblock-labs/sol-chess` — "Solana Chess Engine written in Anchor."

### Why
Our on-chain move validation (`chess-logic-on-chain`, used in
[moves_ix/record.rs](../programs/xfchess-game/src/moves_ix/record.rs)) is the security-critical core.
sol-chess is an independent first-party on-chain chess implementation — an ideal **differential
oracle**: run both over the same move sequences (perft / random legal games) and assert agreement on
legality, check/mate/stalemate, and resulting FEN.

### Integration steps
1. Add a dev-only differential test (workspace test or `crates/.../tests/`) that drives the existing
   perft suite through both `chess-logic-on-chain` and a sol-chess harness.
2. Wire into the existing `comprehensive-testing` plan (see `docs/plans/comprehensive-testing.md`).

### Effort / risk
S. Test-only, no production code. Pure upside — strengthens confidence in the validator we already ship.

---

## 7. (C) session-keys — canonical session delegation  ·  **P3**

**Repo:** `magicblock-labs/session-keys`.

### Why / decision
We maintain **three** bespoke session systems: per-game `SessionDelegation`
([accounts/session_delegation.rs](../programs/xfchess-game/src/accounts/session_delegation.rs)),
`GlobalSessionDelegation`, and tournament sessions. MagicBlock `session-keys` is the maintained
standard. **This is a migration, not an add-on** — only worth it if maintenance of our custom
system becomes a burden, or to align with MagicBlock tooling. Given our global-session work is
complete and audited (see project memory), **defer** unless we hit a concrete limitation.

### If/when adopted
- Replace `authorize_*_session` / `revoke_*_session` ix with session-keys' validation.
- Keep our spending-cap / wager-cap semantics (session-keys is auth, not policy) layered on top.

### Effort / risk
L + behavioural risk to a security-critical, already-working path. Not recommended this cycle.

---

## 8. (D) ephemeral-spl-token — USDC settled on the ER  ·  **P3**

**Repos:** `magicblock-labs/ephemeral-spl-token` (native SPL delegation), `ephemeral-rollups-spl`.

### Why / decision
USDC wagers and tournament prizes (`tournament_ix/prizes/fund_usdc_prize.rs`) currently move on the
**base layer**. If we want USDC stakes to settle with the same sub-second UX as moves, the ER needs
delegated SPL token accounts (ephemeral ATAs). Only pursue if product wants on-ER USDC settlement;
SOL wagers and end-of-game USDC payout work fine today on base layer.

### Effort / risk
L. New token-delegation lifecycle (ephemeral ATA init → delegate → settle → undelegate) parallel to
the game-PDA lifecycle. Defer behind a product decision.

---

## 9. Recommended sequencing

1. **Now (this cycle), on-chain correctness — ✅ DONE & compile-verified (`cargo check -p xfchess-game` green):**
   - ✅ Bumped `ephemeral-rollups-sdk` → 0.13.0; fixed `commit_and_undelegate_accounts` 5th-arg break.
   - ✅ **(A) magic-router prep:** `validator: None`; `commit_frequency_ms` fixed via
     `ER_COMMIT_FREQUENCY_MS`; `address =` constraints added to `magic_context`/`magic_program`.
   - ✅ Deleted dead stubs: `game_ix/commit_move_batch.rs`, `delegation_ix/undelegate.rs`.
   - ⏸ **(B1) crank → `ScheduleCrankCpi`:** **deferred** — the SDK helper serializes via
     `magicblock-magic-program-api 0.8.8` (different wire format than the current `=0.3.1` crank),
     a behavioural change to scheduled tasks that **must be verified on a live ER** first. Not shipped blind.
   - ⚠ **Requires devnet redeploy + test:** the above are code-complete and type-checked, but
     `validator: None` and the new account constraints change on-chain behaviour — **must be
     exercised on devnet** (delegate → record moves → undelegate) before mainnet.
2. **Next: client/backend routing & observability**
   - (A) magic-router in `src/solana/`, `backend/`, `web-solana/`.
   - (E) magicblock-sync in backend.
   - (G) sol-chess differential test.
3. **Later: fairness & product**
   - (F) solana-vrf for color + pairing.
   - (D) ephemeral-spl-token *if* on-ER USDC settlement is wanted.
4. **Deferred / migration-coupled**
   - (C) session-keys (only if custom system becomes a burden).
   - Anchor 1.0 / Solana 3.0 + `ephemeral-rollups-sdk 0.15.x` (unlocks ER-native VRF, eSPL,
     access-control, intent builders) — separate epic.

---

## 10. Open questions

- **magic-router endpoint:** does our existing `magic-resolver 0.2.0` config already expose the
  router URL, or do we need a separate `magic-router-sdk` provider on web? (Verify before §2 step 2.)
- **Mainnet validator policy:** with `validator: None`, who pays/assigns? Confirm cost model vs the
  current `fees_advanced` accounting in `Game`.
- **hydra vs magic-program crank:** is B1 (typed SDK helper) sufficient long-term, or do we want B2
  (hydra) for validator-upgrade resilience before mainnet?
- **VRF cost per game:** acceptable to add a VRF request to the create-game hot path, or only for
  ranked/wagered games?

---

## References

- Org: <https://github.com/orgs/magicblock-labs/repositories>
- SDK: <https://github.com/magicblock-labs/ephemeral-rollups-sdk> (0.13.0 tag)
- Related local docs: `docs/SOLANA_CRATES_AUDIT.md`, `docs/plans/comprehensive-testing.md`,
  `docs/plans/auth-and-program-id-hardening.md`
