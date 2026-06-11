# Plan: Tournament On-Chain Wiring + Blinks Integration

## Problem

The tournament system is split in two: the **Solana program** has a complete lifecycle
(`initialize → escrow → shards → register → start → match → record_result → claim_prize`)
but the **backend never calls any of it**. Tournament creation writes only to the in-memory
store. Entry fees are not collected on-chain. Prize money never exists in escrow. If the
backend restarts during a live tournament, all state is gone.

The blinks layer (`backend/src/signing/blinks/`) already has:
- `GET /api/actions/tournament/:id` — action metadata ✓
- `POST /api/actions/tournament/:id/register` — builds a register tx, but with **wrong PDAs**
  (missing shard accounts required by `register.rs` in the program)
- Validate / balance-check / action-chain endpoints ✓

What does not exist:
- On-chain tournament creation fired from the admin CLI
- Correct `register_player` instruction (shard accounts missing)
- Store update after a confirmed registration tx
- On-chain start + match initialization
- On-chain result recording
- A `claim_prize` blink for winners

---

## Deliverables

| # | File | Change |
|---|------|--------|
| 1 | `backend/src/signing/solana/instructions.rs` | Add `initialize_escrow_ix`, `initialize_shards_ix`, `start_tournament_ix`, `initialize_match_ix`, `record_result_ix`, `claim_prize_ix` |
| 2 | `backend/src/signing/blinks/core.rs` | Fix `build_register_transaction` — add correct shard accounts; add `build_claim_prize_transaction`; add `build_start_tournament_transaction` |
| 3 | `backend/src/signing/blinks/routes.rs` | Add `POST /api/actions/tournament/:id/claim-prize`, `GET /api/actions/tournament/:id/claim-prize`, `POST /api/actions/admin/tournament/:id/start` |
| 4 | `backend/src/signing/routes/tournament.rs` | `create_tournament`: fire 3 sequential Solana txs (VPS-signed) before store write; `record_result`: call `record_result_ix` after store update |
| 5 | `backend/src/tasks/` | Scheduler: after auto-start, call `start_tournament_ix` + one `initialize_match_ix` per match slot |

---

## Phase 1 — Instruction builders

**File:** `backend/src/signing/solana/instructions.rs`

### `initialize_escrow_ix`
Seeds: `[b"tournament_escrow", tournament_id_le]`  
Signers: VPS authority (already in `AppState` as `vps_authority`)

```rust
pub fn initialize_escrow_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    authority: &Pubkey,
) -> Instruction
```

### `initialize_shards_ix`
Three variants keyed off `max_players`:
- ≤ 64 → `initialize_shards_small` (1 shard PDA, seed `[b"t_players", [0u8], id_le]`)
- ≤ 128 → `initialize_shards_medium` (shards 0+1)
- 256 → `initialize_shards_large` (shards 0–3)

```rust
pub fn initialize_shards_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    authority: &Pubkey,
) -> Instruction
```

### `start_tournament_ix`
Calls `start_tournament`. Passes all 4 shard PDAs (program ignores ones it doesn't need).

```rust
pub fn start_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    authority: &Pubkey,
) -> Instruction
```

### `initialize_match_ix`
One call per bracket slot. Seeds: `[b"t_match", tournament_id_le, match_index_le]`.

```rust
pub fn initialize_match_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    match_index: u16,
    authority: &Pubkey,
) -> Instruction
```

### `record_result_ix`
VPS-signed. Resolves a `TournamentMatch` PDA to completed, advances winner.

```rust
pub fn record_result_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    match_index: u16,
    winner: &Pubkey,
    loser: &Pubkey,
    game_pda: &Pubkey,
    authority: &Pubkey,
) -> Instruction
```

### `claim_prize_ix`
Player-signed. Pulls SOL from escrow PDA to winner's wallet.

```rust
pub fn claim_prize_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    claimant: &Pubkey,
) -> Instruction
```

---

## Phase 2 — Fix register blink + add claim/start blinks

**File:** `backend/src/signing/blinks/core.rs`

### Fix `build_register_transaction`

The current instruction builder passes 8 accounts but the program's `RegisterPlayer`
context requires the player shard PDAs. The correct account list (matching
`programs/xfchess-game/src/tournament_ix/registration/register.rs`):

```
tournament PDA         writable
player_profile PDA     readonly
player                 writable, signer
escrow_pda             writable
shard_0 PDA            writable      (always present)
shard_1 PDA            writable opt  (≥128 players)
shard_2 PDA            writable opt  (256 players only)
shard_3 PDA            writable opt  (256 players only)
system_program         readonly
```

Shard PDAs are derived from `blinks/pda.rs` — add:

```rust
pub fn derive_shard_pda(shard_index: u8, tournament_id: u64, program_id: &Pubkey) -> Result<Pubkey>
```

After the transaction is built, the route handler must call `store.update(id, ...)` to
add the player to the in-memory record (confirmation callback pattern: fire tx, wait for
confirmation, then mutate store — or optimistic: mutate store immediately and revert on
timeout).

### New: `build_claim_prize_transaction`

Returns a base64 unsigned tx. The claimant signs it client-side via their wallet.
Blinks-style response: `{ transaction, message }`.

### New: `build_start_tournament_transaction`

Admin-only. Fires `start_tournament_ix` + all `initialize_match_ix` calls in a single
batch (or multiple txs for large brackets — 128p needs 127 match PDAs, too many for one
tx; batch into groups of 20).

---

## Phase 3 — Route changes

**File:** `backend/src/signing/routes/tournament.rs`

### `create_tournament` handler

After parsing the request and before writing to the store, fire three sequential
VPS-signed transactions:

```
1. initialize_tournament_ix  → confirmed
2. initialize_escrow_ix      → confirmed
3. initialize_shards_ix      → confirmed (variant chosen by max_players)
```

Use `state.vps_authority` to sign. Use `solana::make_rpc(&state.config.solana_rpc_url)`.

If any tx fails, return `500` without writing to the store.

### `record_result` handler

After `store.record_result(...)` succeeds, look up the match's `game_id` and fire
`record_result_ix` (VPS-signed). Log the signature. Do not fail the HTTP response
if the on-chain call fails — log an error and emit a Prometheus metric
(`tournament_result_onchain_failed_total`).

---

## Phase 4 — New blink endpoints

**File:** `backend/src/signing/blinks/routes.rs`

### `GET /api/actions/tournament/:id/claim-prize?wallet=<pubkey>`

Returns `ActionMetadata` showing the player's prize amount from `prize_shares`.
Only valid if tournament status is `Completed` and `wallet` matches a placed finisher.

### `POST /api/actions/tournament/:id/claim-prize`

Body: `{ "account": "<pubkey>" }`  
Returns `{ "transaction": "<base64>", "message": "Claim your prize" }`  
Builds `claim_prize_ix` — player signs and broadcasts.

### `POST /api/actions/admin/tournament/:id/start`

Requires `X-API-Key` header. Calls `build_start_tournament_transaction` (batched if
needed). Returns `{ "ok": true, "signatures": [...] }` after all txs confirm.
Wires into the existing scheduler trigger path so auto-start also calls this.

---

## Phase 5 — Scheduler wiring

**File:** `backend/src/tasks/` (whichever file handles `TournamentTrigger::PlayerJoined`)

When auto-start fires (tournament full or `scheduled_at` reached):
1. Call `start_tournament_ix` (VPS-signed) — on-chain bracket lock
2. For each match slot (0 to `max_players - 2`), call `initialize_match_ix` in batches
3. Only after all match PDAs confirmed, set store status to `Active` and begin pairing

---

## On-chain flow after full wiring

```
Admin CLI → create_tournament
    └─ VPS signs: initialize_tournament → initialize_escrow → initialize_shards
           └─ Tournament PDA + Escrow PDA live on-chain

Player → POST /api/actions/tournament/:id/register (blink)
    └─ Returns unsigned tx
    └─ Player wallet signs: register_player → SOL moves to escrow PDA
           └─ store.update adds player

Scheduler → tournament full / scheduled_at
    └─ VPS signs: start_tournament → initialize_match × N
           └─ Bracket locked on-chain

Admin CLI option 5 / auto-advance
    └─ VPS signs: record_result(match_index, winner, loser)
           └─ TournamentMatch PDA updated on-chain

Player → GET /api/actions/tournament/:id/claim-prize (blink)
    └─ Returns unsigned tx
    └─ Player wallet signs: claim_prize → SOL pulled from escrow to winner wallet
```

---

## Notes

- `vps_authority` is the only key the backend holds. All player-facing txs are returned
  unsigned for the player's wallet to sign — the backend never touches player funds.
- For the shard account mismatch: existing blink tests will break once Phase 2 is
  deployed. Run `cargo test -p backend` after each phase.
- 128p weekend tournaments need 127 `initialize_match_ix` calls — batch 20 per tx =
  7 transactions. At ~5000 lamports each that's ~0.00035 SOL overhead per tournament.
- Prize currency: the program supports both SOL (escrow PDA) and USDC (token account).
  Phase 1–5 wires SOL only. USDC path can follow when the USDC mint is configured.
