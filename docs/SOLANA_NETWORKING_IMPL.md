# Solana Networking — Implementation Plan

Each item is self-contained. Work them in order — each one builds on the last's infrastructure.

---

## 1. `finalize_game` e2e — Prize claimed UI

**What you get:** After checkmate/resign the game-over popup shows "Prize Claimed ✓" with the
exact SOL amount transferred on-chain, instead of a blank or placeholder value.

### What exists

- `bridge.rs` → `spawn_finalization_task` already calls `vps_undelegate_game` then
  `vps_finalize_game` in a background thread.
- `game_over_popup.rs` → `GameOverPayoutInfo` resource exists but is never populated with real
  on-chain data — `winning_prize` is always 0.
- `vps/game.rs` → `vps_finalize_game` returns `Ok(sig)` but the signature is just logged, not
  surfaced to the UI.

### What to build

**`src/multiplayer/rollup/bridge.rs`**

In `spawn_finalization_task`, after `vps_finalize_game` succeeds, send a one-shot channel result
back to the Bevy world carrying `{ sig, winner_lamports }`. Add a system
`apply_finalization_result` that reads the channel, inserts/mutates `GameOverPayoutInfo` with the
real prize amount, and sets a new flag `payout_confirmed: true`.

```rust
// bridge.rs — extend spawn_finalization_task return value
pub struct FinalizationResult {
    pub sig: String,
    pub winner_lamports: u64, // comes from VPS response body
}
```

The VPS `/game/finalize` endpoint already knows `wager_amount` from the Game account — extend its
JSON response to include `{ sig, winner_lamports, country_fee }`.

**`backend/src/signing/routes/main.rs`** — add `winner_lamports` and `country_fee` to the
`FinalizeResponse` struct so the client can read them without a separate RPC call.

**`src/ui/menus/game_over_popup.rs`**

Add a "Prize" row in the popup that renders only when `payout_info.payout_confirmed`:

```
┌────────────────────────────────┐
│        White Wins!             │
│  Prize claimed  0.019 SOL ✓   │  ← new row
│  Tx: 3xK…mPq  [explorer↗]    │  ← clickable sig
│         [Main Menu]            │
└────────────────────────────────┘
```

Use `open::that(explorer_url)` (already a dep) to open the Solana explorer link.

---

## 2. Undelegation gate — poll `delegated` flag before `finalize_game`

**What you get:** `finalize_game` never fails with "account still owned by ER" because the client
waits for the account to be fully back on devnet before proceeding.

### What exists

`bridge.rs` `spawn_finalization_task` currently sleeps a fixed 10 s after `vps_undelegate_game`
then calls `vps_finalize_game`. This races with MagicBlock's propagation time — on a slow relay
the finalize will fail because the account is still ER-owned.

The `Game` account has a `delegated: bool` field visible via RPC. `DelegationRecord` PDA
(owned by `DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh`) also stores delegation status.

### What to build

**`src/multiplayer/rollup/bridge.rs`** — replace the fixed `sleep(10s)` with a poll loop:

```rust
// After vps_undelegate_game succeeds, poll devnet RPC until game account
// is owned by the program again (not the delegation program).
async fn wait_for_undelegation(rpc: &RpcClient, game_pda: Pubkey) -> Result<(), String> {
    for attempt in 0..30 {                         // 30 × 2s = 60s max
        tokio::time::sleep(Duration::from_secs(2)).await;
        match rpc.get_account(&game_pda) {
            Ok(acc) if acc.owner == PROGRAM_ID => return Ok(()),
            Ok(_) => {}   // still ER-owned
            Err(_) => {}  // transient — keep polling
        }
        if attempt == 15 {
            warn!("[UNDELEGATE] Still waiting after 30s for game {} to land on devnet", game_pda);
        }
    }
    Err(format!("game {} never returned to devnet after undelegation", game_pda))
}
```

Replace the existing `sleep(10)` call with `wait_for_undelegation(base_rpc, game_pda).await?`.

Add a timeout fallback: if 60 s pass without the account returning, still attempt finalize and
log a warning — never block the user forever.

---

## 3. Session key expiry UX — prompt re-authorize if < 24 h remaining

**What you get:** Players are warned before a game starts that their session key is about to
expire, and can tap one button to re-authorize without leaving the lobby.

### What exists

- `session_key_manager.rs` stores `expires_at: i64` in the encrypted disk record.
- `GameStartedEvent { game_id }` fires in `bridge.rs` when the game enters `InProgress`.
- The `SessionKeyManager` is loaded in `integration/systems.rs` but `expires_at` is never
  checked at game-start time.

### What to build

**`src/multiplayer/solana/integration/systems.rs`** — add a system
`check_session_expiry_on_game_start` that runs `on(GameStartedEvent)`:

```rust
fn check_session_expiry_on_game_start(
    mut events: EventReader<GameStartedEvent>,
    session_mgr: Res<SessionKeyManager>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let Some(expires_at) = session_mgr.expires_at() else { continue };
        let remaining_secs = expires_at - Utc::now().timestamp();
        if remaining_secs < 86_400 {        // < 24 h
            commands.insert_resource(SessionExpiryWarning {
                game_id: ev.game_id,
                expires_in_hours: (remaining_secs / 3600) as u32,
            });
        }
    }
}

#[derive(Resource)]
pub struct SessionExpiryWarning { pub game_id: u64, pub expires_in_hours: u32 }
```

**`src/multiplayer/solana/session_key_manager.rs`** — add `pub fn expires_at(&self) -> Option<i64>`
that reads `session_data.expires_at` from the cached in-memory record.

**`src/ui/menus/`** — add `session_expiry_banner.rs` (egui toast at top of game screen):

```
┌─────────────────────────────────────────────────────┐
│ ⚠  Session key expires in 3 h.  [Re-authorize]      │
└─────────────────────────────────────────────────────┘
```

[Re-authorize] calls `integration/systems.rs` `authorize_session_key` flow which already exists —
just trigger it from the banner button. Remove `SessionExpiryWarning` resource when re-auth
succeeds.

---

## 4. Free Rated ELO submission — POST `/ratings/update` after game

**What you get:** Free Rated games (no wager) update the on-chain ELO and appear in the player's
rating history immediately after the game ends, not only after a wager game is finalized.

### What exists

- `GameEndedEvent { game_id, winner, reason }` fires in `bridge.rs`.
- `vps/game.rs` has `vps_finalize_game` which updates ELO — but it is only called when the game
  is delegated (i.e., was a proper wager game routed through ER).
- Free Rated games are played through the VPS relay but are never finalized on-chain at all.

### What to build

**`src/multiplayer/network/vps/game.rs`** — add:

```rust
#[derive(Serialize)]
struct FreeRatedResultReq<'a> {
    game_id: u64,
    winner: Option<&'a str>,   // "white" | "black" | null (draw)
    white_pubkey: &'a str,
    black_pubkey: &'a str,
}

pub fn vps_submit_free_rated_result(
    game_id: u64,
    winner: Option<&str>,
    white_pubkey: &str,
    black_pubkey: &str,
) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/ratings/update", vps_base()))
        .json(&FreeRatedResultReq { game_id, winner, white_pubkey, black_pubkey })
        .send()
        .map_err(|e| format!("ratings/update: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("ratings/update: HTTP {}", response.status()));
    }
    Ok(())
}
```

**`backend/src/signing/routes/`** — `ratings.rs` already exists; add `POST /ratings/update`
handler that reads `GameState` from DB (or in-memory), validates winner is plausible, calls
`update_elo_on_chain` (already used in finalize path), responds `200 {}`.

**`src/multiplayer/rollup/bridge.rs`** `handle_game_end_undelegation`:

```rust
// After determining winner, before spawn_finalization_task:
if !is_delegated {
    // Free Rated path — no ER, no escrow, just ELO update
    let w = white_pk.to_string();
    let b = black_pk.to_string();
    let win = winner.clone();
    std::thread::spawn(move || {
        if let Err(e) = vps_submit_free_rated_result(game_id, win.as_deref(), &w, &b) {
            error!("[FREE_RATED] ELO update failed: {e}");
        }
    });
    return; // done — no on-chain finalize needed
}
```

You need a way to know a game is Free Rated vs Wager. `SolanaIntegrationState` should carry
`match_type: u8` (already in `Game` account on-chain); read it at game-start from the RPC fetch
in `integration/systems.rs`.

---

## 5. `record_move` nonce resync — fetch on-chain nonce on reconnect

**What you get:** Resuming a game after a disconnect never produces "invalid nonce" rejections on
the ER because the client re-derives the true nonce from the live `MoveLog` account.

### What exists

`bridge.rs` `move_nonce` starts at 1 and increments by 1 per move recorded. On reconnect the
resource is re-initialized to 1 — but the on-chain `MoveLog.nonce` may already be at e.g. 47.

The `MoveLog` PDA seed is `[b"move_log", &game_id.to_le_bytes()]`. Its layout has `nonce: u64`
at a known offset (after the 8-byte Anchor discriminator and game_id field).

### What to build

**`src/multiplayer/network/vps/game.rs`** — add:

```rust
pub fn vps_fetch_move_nonce(game_id: u64) -> Result<u64, String> {
    // GET /game/{game_id}/nonce — lightweight endpoint that just reads MoveLog.nonce
    let response = client()?
        .get(format!("{}/game/{}/nonce", vps_base(), game_id))
        .send()
        .map_err(|e| format!("fetch_nonce: {e}"))?;
    #[derive(serde::Deserialize)]
    struct NonceResp { nonce: u64 }
    Ok(response.json::<NonceResp>().map_err(|e| e.to_string())?.nonce)
}
```

**`backend/src/signing/routes/main.rs`** — add `GET /game/:id/nonce`:

```rust
async fn get_game_nonce(Path(game_id): Path<u64>, State(ctx): State<AppCtx>) -> Json<Value> {
    let pda = move_log_pda(game_id);
    let acc = ctx.rpc.get_account(&pda)?;
    let nonce = u64::from_le_bytes(acc.data[16..24].try_into().unwrap()); // after disc + game_id
    Json(json!({ "nonce": nonce }))
}
```

**`src/multiplayer/rollup/bridge.rs`** — in `handle_game_start` (or wherever `GameStartedEvent`
is consumed), spawn a task to fetch the nonce and write it back to `RollupNetworkBridge.move_nonce`:

```rust
// In handle_game_start or on reconnect detection:
let (tx, rx) = oneshot::channel::<u64>();
std::thread::spawn(move || {
    if let Ok(nonce) = vps_fetch_move_nonce(game_id) {
        let _ = tx.send(nonce + 1); // next expected nonce
    }
});
bridge.nonce_rx = Some(rx);

// In Update system:
if let Some(rx) = &mut bridge.nonce_rx {
    if let Ok(nonce) = rx.try_recv() {
        bridge.move_nonce = nonce;
        bridge.nonce_rx = None;
    }
}
```

---

## 6. Dispute UI — Dispute button in post-game screen for wager games

**What you get:** After a wager game the loser has a "Dispute Result" button. Clicking it submits
a `dispute` instruction on-chain and opens a 48-hour arbitration window visible in the admin panel.

### What exists

- `backend/src/signing/routes/dispute.rs` exists — has `POST /dispute/submit` that builds the
  `dispute` instruction.
- `game_over_popup.rs` has `GameOverPayoutInfo.is_wager_game()`.
- The program has `governance_ix/dispute.rs` with a `Dispute` accounts struct.

### What to build

**`src/multiplayer/network/vps/game.rs`** — add:

```rust
pub fn vps_submit_dispute(game_id: u64, disputing_player: &str) -> Result<String, String> {
    // POST /dispute/submit → returns { sig }
}
```

**`src/ui/menus/game_over_popup.rs`** — show the button only when:
- `payout_info.is_wager_game()` is true
- local player is the loser (`payout_info.player_color` != winner color)
- `payout_info.payout_confirmed` is true (settlement already happened on-chain)

```rust
if payout_info.is_wager_game() && player_is_loser {
    if ui.button("⚠ Dispute Result").clicked() {
        commands.insert_resource(PendingDispute { game_id });
    }
}
```

Add `PendingDispute` resource → system in `bridge.rs` picks it up, calls `vps_submit_dispute`,
removes the resource on success, shows a toast "Dispute submitted — 48 h review window open".

**Constraint:** Only show Dispute within 48 h of game end. Store `game_ended_at: SystemTime` in
`GameOverPayoutInfo` and gate on `elapsed < Duration::from_secs(48 * 3600)`.

---

## 7. Tournament session routing — detect tournament match, use session instructions

**What you get:** Players in a tournament never get a "create game" popup — the game is created
transparently via their pre-authorized tournament session key with a single VPS-signed transaction.

### What exists

- `src/multiplayer/solana/tournament.rs` receives `SwissMessage::RoundStarted { pairings }` via
  Braid gossip and stores `TournamentState { pairings: Vec<(String, String)> }`.
- `src/multiplayer/solana/tournament_session.rs` has all the instruction builders for
  `session_create_game` and `session_join_game`.
- Normal game creation goes through `vps_client::create_game` (standard path).
- The two paths have never been wired together.

### What to build

**`src/multiplayer/solana/tournament.rs`** — when a pairing is received that includes the local
player's pubkey, emit a new event:

```rust
#[derive(Event)]
pub struct TournamentMatchAssigned {
    pub tournament_id: u64,
    pub game_id: u64,          // derived deterministically from (tournament_id, round, board)
    pub is_white: bool,
    pub opponent: Pubkey,
}
```

**`src/multiplayer/rollup/bridge.rs`** (or a new `tournament_bridge.rs`) — system on
`TournamentMatchAssigned`:

```rust
fn handle_tournament_match(
    mut events: EventReader<TournamentMatchAssigned>,
    session_mgr: Res<TournamentSessionManager>,
    mut commands: Commands,
) {
    for ev in events.read() {
        if ev.is_white {
            // White creates — POST /tournament/session_create_game
            spawn_session_create_game(ev.tournament_id, ev.game_id, session_mgr.session_key());
        } else {
            // Black joins — wait 3 s for white to create, then POST /tournament/session_join_game
            spawn_session_join_game_with_backoff(ev.tournament_id, ev.game_id, ev.opponent, session_mgr.session_key());
        }
        // Set a flag so the normal lobby creation path is skipped
        commands.insert_resource(TournamentGameContext { tournament_id: ev.tournament_id, game_id: ev.game_id });
    }
}
```

**`backend/src/signing/routes/`** — add `POST /tournament/session_create_game` and
`POST /tournament/session_join_game` handlers that call the existing instruction builders
in `backend/src/signing/solana/instructions.rs` using the VPS `vps_authority` keypair as
the session co-signer, then broadcast to devnet.

**Game detection gate** — in `handle_game_end_undelegation` and the record-move path, check for
`TournamentGameContext` resource to determine that `record_swiss_result` (not `finalize_game`)
is the correct post-game instruction.

---

## 8. Global session VPS handshake — full round-trip verification

**What you get:** Verified that the "authorize once, play forever" flow actually works end-to-end:
client generates a global session key → backend co-signs the authorization tx → player broadcasts
→ every subsequent game creation requires zero wallet popups.

### What exists

- `global_session_manager.rs` has `build_authorize_global_session_ix`,
  `build_global_create_game_ix`, `GlobalSessionKeyManager::load_or_create`, `save`.
- `backend/src/signing/routes/global_session.rs` exists — has at least a stub.
- `integration/systems.rs` calls `authorize_session_key` per-game but never the global path.

### What to build

This is an integration test / smoke test, not new code. The round-trip has five checkpoints:

| # | Step | Success condition |
|---|------|------------------|
| 1 | `GlobalSessionKeyManager::load_or_create(wallet)` | Returns keypair, saves `.enc` to disk |
| 2 | Client POSTs `{ session_pubkey, wallet_pubkey, duration_days: 30 }` to `POST /sign/global_create` | Backend returns `{ tx_b64: "..." }` (partially signed by VPS) |
| 3 | Client deserializes tx, signs with wallet, broadcasts to devnet | `get_signature_statuses` confirms in < 60 s |
| 4 | `GET /sign/global_verify?wallet=<pubkey>` | Backend reads PDA, returns `{ active: true, expires_at }` |
| 5 | Client creates a game via `build_global_create_game_ix` without wallet popup | Game PDA exists on devnet |

**`backend/src/signing/routes/global_session.rs`** — implement if stubbed:

```rust
// POST /sign/global_create
async fn global_create_session(
    Json(req): Json<GlobalCreateReq>,   // { session_pubkey, wallet_pubkey, duration_days }
    State(ctx): State<AppCtx>,
) -> Result<Json<TxResp>, AppError> {
    let ix = build_authorize_global_session_ix(
        &PROGRAM_ID, &req.wallet_pubkey, &req.session_pubkey,
        req.duration_days * 86_400,     // duration_secs
        None,                           // spending_limit
    );
    let tx = build_and_partial_sign(&ctx.rpc, &[ix], &ctx.vps_keypair).await?;
    Ok(Json(TxResp { tx_b64: base64_encode(tx) }))
}

// GET /sign/global_verify?wallet=<pubkey>
async fn global_verify(Query(q): Query<VerifyQuery>, State(ctx): State<AppCtx>) -> Json<Value> {
    let (pda, _) = find_global_session_pda(&PROGRAM_ID, &q.wallet);
    match ctx.rpc.get_account(&pda) {
        Ok(_) => Json(json!({ "active": true })),
        Err(_) => Json(json!({ "active": false })),
    }
}
```

**`src/multiplayer/solana/integration/systems.rs`** — at `AppState::MainMenu` entry, check if
`GlobalSessionKeyManager::load(wallet).is_ok()` and PDA is active; if not, show one-time
"Authorize XFChess" button that triggers the full 5-step handshake above. Once confirmed, set
`GlobalSessionActive` resource so the game never asks again.

**Test script** — add `src/bin/global_session_test.rs` that runs checkpoints 1-5 sequentially
and prints pass/fail per step (mirrors the `on_chain_benchmark.rs` pattern already in the repo).

---

## Completion order

```
1 → 2 (prize UI then gate that makes it reliable)
3       (independent, ship any time)
4       (independent — needs match_type in SolanaIntegrationState)
5       (independent — needed for resilient reconnects)
6       (needs 1 done first so payout_confirmed exists)
7       (needs 4 done — tournament games are free rated)
8       (last — integrates all of the above)
```
