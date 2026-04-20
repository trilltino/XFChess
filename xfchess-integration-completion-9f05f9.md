# XFChess Full Integration Completion Plan

Complete the integration of Solana, multiplayer, and MagicBlock ER features by fixing 38 compilation errors, configuring the VPS backend, and establishing end-to-end PvP wager game flow with on-chain escrow and ER delegation.

## Current Status Assessment

### ✅ Completed
- Fixed `CommitMoveBatch` missing `nonce_start` field in both client and instruction builder
- Renamed `multiplayer_disabled` → `multiplayer`
- Enabled `solana` feature in default build
- Uncommented `pub mod multiplayer` in `lib.rs`
- Updated `MainMenuUIContext` with multiplayer/solana fields
- Fixed tournament client borrow error

### ❌ Remaining (38 errors)
- Missing `PeerInfo` fields (`connected_game`, `role`) at 2 locations
- Missing `render_create_tab` and `render_join_tab` functions in main_menu.rs
- Bevy API issues (`add_event`, `add_message` method not found)
- Import/Module path errors
- Missing UI components for wallet integration

### 🔧 Infrastructure Needed
- VPS backend needs proper deployment/configuration
- Tauri wallet integration needs verification
- Solana program deployment to devnet

---

## Phase 1: Fix Remaining Compilation Errors (Priority: HIGH)

### 1.1 Fix PeerInfo Struct Initializers
**Files:** `src/multiplayer/mod.rs:318`, `src/multiplayer/mod.rs:513`

Add missing fields to `PeerInfo` initializers:
```rust
PeerInfo {
    node_id: bs58_id.clone(),
    wallet_address: format!("sol:{}...", &bs58_id[..8]),
    game_preferences: GamePreferences { ... },
    last_seen: Instant::now(),
    role: NodeRole::Peer,  // ADD THIS
    connected_game: None,  // ADD THIS
}
```

### 1.2 Implement Missing UI Functions
**File:** `src/states/main_menu.rs`

Create missing `render_create_tab` and `render_join_tab` functions for the lobby UI, or stub them out:
```rust
fn render_create_tab(ui: &mut egui::Ui, lobby: &mut SolanaLobbyState, compliance: &mut ComplianceState) {
    // UI for creating a new game with wager
}

fn render_join_tab(ui: &mut egui::Ui, lobby: &mut SolanaLobbyState) {
    // UI for joining an existing game by ID
}
```

### 1.3 Fix Bevy API Calls
**Files:** `src/multiplayer/tournament/events.rs:102`, `src/multiplayer/mod.rs:64`

Change `app.add_event::<T>()` to `app.add_event::<T>()` → Actually this should work in Bevy 0.18.
Alternative: May need to import `bevy::app::App` correctly or use `app.add_event` via `Plugin` trait.

Investigate if `add_message` should be `add_event` for message types.

### 1.4 Fix Module Import Errors
**Various files**

Resolve `E0432`, `E0433` import errors - likely missing module declarations or incorrect paths.

---

## Phase 2: Infrastructure Setup (Priority: HIGH)

### 2.1 Backend Deployment

**Option A: Local Development Backend**
```bash
cd backend
cargo run
# Configure .env with:
# - SOLANA_RPC_URL=https://api.devnet.solana.com
# - PROGRAM_ID=your_deployed_program_id
# - FEE_PAYER_KEYPAIR=path_to_keypair.json
```

**Option B: Deploy to VPS**
- Hetzner/DigitalOcean VM
- Docker container with backend
- Configure domain/subdomain
- Update client `vps_client.rs` with actual backend URL

### 2.2 Solana Program Deployment

**Devnet Deployment:**
```bash
cd programs/xfchess-game
anchor build
anchor deploy --provider.cluster devnet
# Update PROGRAM_ID in:
# - src/solana/constants.rs
# - Cargo.toml workspace.dependencies
```

### 2.3 Tauri Wallet Setup

**Verify Tauri Build:**
```bash
cd tauri
cargo tauri build
# Test wallet popup with Phantom
```

**Configuration:**
- Ensure `tauri.conf.json` has correct permissions
- Verify `wallet-ui/` React app builds correctly
- Test wallet adapter connection

---

## Phase 3: Core PvP Flow Implementation (Priority: MVP)

### 3.1 Wallet Connection Flow

**UI:** Add wallet connection button to main menu
**Flow:**
1. User clicks "Connect Wallet"
2. Tauri opens popup with Phantom/Solflare options
3. Wallet connects and returns pubkey
4. `SolanaIntegrationState` populated with wallet info
5. Wager game buttons become enabled

### 3.2 Game Creation Flow

**Host:**
1. User sets wager amount, clicks "Create Game"
2. Frontend calls `spawn_create_game()`:
   - Generate `game_id` (random u64)
   - Call `vps_client::create_session(game_id, wallet_pubkey)` → get session_pubkey
   - Build `create_game_ix` with wager amount
   - Build `authorize_session_key_ix` for delegation
   - Tauri signing popup for both instructions
   - On success: spawn Iroh node
   - Call `vps_client::p2p_announce()` to advertise game
3. Game appears in VPS game list
4. Host waits in lobby screen, polling for joiner

### 3.3 Game Discovery & Join Flow

**Joiner:**
1. Poll `GET /p2p/games` → display game list
2. User selects game, clicks "Join"
3. Frontend calls `spawn_join_game()`:
   - Call `join_game_ix` on-chain (wallet popup)
   - Call `authorize_session_key_ix` (session key auth)
   - Call `vps_client::p2p_join_game()` with joiner_node_id
   - VPS returns `host_node_id`
   - Iroh gossip connects to host_node_id
4. Game starts

### 3.4 Gameplay Flow

**Move Transmission:**
1. Player makes move
2. Local validation (legal move check)
3. Send via Iroh gossip to opponent
4. Opponent receives, validates, applies to board
5. Both players record move in ER (via session key, no popup)

**Batching:**
- Every N moves or T seconds, commit batch to ER
- `commit_move_batch_ix` with session key signing
- Continue gameplay

### 3.5 Game Finalization

**End Game Detection:**
- Checkmate, stalemate, timeout, or resign detected

**Finalize:**
1. Call `undelegate_game` instruction (session key)
2. Wait for undelegation to base layer
3. Call `finalize_game` instruction (session key)
4. On-chain payout to winner

---

## Phase 4: Tournament Implementation (Priority: POST-MVP)

### 4.1 Tournament Discovery
- `GET /api/tournaments` endpoint
- UI shows tournament list with entry fees, prizes, status

### 4.2 Tournament Registration
1. User clicks "Register" on tournament
2. On-chain `register_player` instruction (wallet popup)
3. `POST /tournament/{id}/subscribe-node` with node_id
4. Backend returns bootstrap peers
5. Connect to gossip mesh

### 4.3 Tournament Match Play
Same as PvP flow, but:
- Matches triggered by bracket advancement
- Results recorded on-chain via `record_match_result`
- Bracket advances via `advance_winner` crank

---

## Phase 5: Testing & Verification

### 5.1 Unit Tests
```bash
cargo test -p xfchess-game  # Program tests
cargo test -p xfchess       # Client tests
```

### 5.2 Integration Tests

**Test 1: PvP Wager Game**
1. Player A creates game with 0.01 SOL wager
2. Player B discovers and joins game
3. Play 10 moves
4. Player A checkmates
5. Verify on-chain payout

**Test 2: Tournament**
1. Create 4-player tournament
2. 4 players register
3. Play semifinals + final
4. Verify winner receives prize

### 5.3 Manual UI Tests
- Wallet connection flow
- Game creation UI
- Game discovery UI
- Move transmission speed (< 100ms)
- Session key popup only at delegation (not per move)

---

## Phase 6: Deployment

### 6.1 Program Deployment
- Deploy to devnet for testing
- (Future) Deploy to mainnet for production

### 6.2 Backend Deployment
- Docker containerization
- VPS deployment (Hetzner/DigitalOcean)
- SSL certificate
- Environment variables configured

### 6.3 Client Release
- Tauri build for Windows
- Installer generation
- Auto-updater configuration

---

## Decision Points

### Backend URL Configuration
**Current:** `https://unrejuvenated-philologically-trudi.ngrok-free.dev`
**Decision:** 
- Option A: Deploy backend to permanent VPS
- Option B: Use ngrok for development (temporary)
- Option C: Local backend for development only

### Solana Network
**Recommendation:** Devnet for all development/testing
- Faster block times (400ms)
- Free SOL from faucet
- No real money at risk

### Tauri Integration
**Decision:** Include in MVP
- Wallet popups are essential for user experience
- Tauri provides clean webview-based UI
- Alternative (native Solana SDK) is more complex

---

## File Changes Summary

### Critical Files to Modify
| File | Changes |
|------|---------|
| `src/multiplayer/mod.rs` | Fix PeerInfo initializers |
| `src/states/main_menu.rs` | Add render_create_tab, render_join_tab |
| `src/multiplayer/tournament/events.rs` | Fix add_event API |
| `src/ui/system_params/main_menu.rs` | ✅ Already updated |
| `src/solana/instructions.rs` | ✅ Already updated |
| `src/multiplayer/vps_client.rs` | Update backend URL |
| `src/solana/constants.rs` | Update PROGRAM_ID |

### New Files to Create
| File | Purpose |
|------|---------|
| `src/states/main_menu_lobby.rs` | Lobby UI components (if not in main_menu.rs) |
| `.env` | Backend configuration |

---

## Success Criteria

- [ ] `cargo build` succeeds with 0 errors
- [ ] `cargo test` passes
- [ ] Wallet connects via Tauri popup
- [ ] Can create PvP game with on-chain escrow
- [ ] Game appears in lobby/game list
- [ ] Second player can discover and join game
- [ ] Moves transmit via Iroh P2P
- [ ] Session keys auto-authorize
- [ ] Game finalizes on-chain with correct payout
- [ ] Tournament creation and registration works

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Session key security | Ephemeral per-game, auto-revoke on end |
| P2P connection fails | VPS relay fallback |
| Wallet popup blocked | Retry logic + clear error messages |
| ER delegation fails | Fallback to base layer moves |
| Backend downtime | Local PvP mode always available |
| Compilation complexity | Iterative fixes, test each phase |

---

## Recommended Next Actions (Immediate)

1. **Fix PeerInfo fields** - 5 min
2. **Implement render_create_tab/render_join_tab stubs** - 10 min
3. **Run cargo check** - verify error count reduced
4. **Fix remaining Bevy API issues** - 15 min
5. **Test compile** - should reach 0 errors
6. **Configure backend URL** - 5 min
7. **Test wallet connection** - 10 min

**Estimated time to first working build:** 2-3 hours
**Estimated time to full PvP flow:** 1-2 days
