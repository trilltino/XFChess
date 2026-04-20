# XFChess Tournament System — Complete Expansion Plan

**Status:** Planning Phase  
**Target:** Swiss pairing integration with Braid-based scheduling and Spectator UI  
**Estimated Timeline:** 4-6 weeks (phased approach)

---

## Phase 1: Core Data Structure Refactoring (Week 1)

### 1.1 Expand TournamentType Enum (On-Chain)
**File:** `programs/xfchess-game/src/state/tournament.rs`

Add new variants to support all tournament formats:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum TournamentType {
    SingleElimination,
    Swiss { rounds: u8 },
}
```

**Changes Required:**
- Keep existing `TournamentType` (already has Swiss and SingleElimination)
- Update `total_matches` calculation:
  - SingleElimination: `n - 1` matches (bracket tree)
  - Swiss: `rounds * floor(n/2)` matches

### 1.2 Promote Fixed Match Array to Vec (Backend)
**Files:**
- `backend/src/tournament/store.rs`
- `backend/src/tournament/routes.rs`

**Current:** `matches: [Option<TournamentMatch>; 3]` (hard-coded for 4-player bracket)

**New Structure:**
```rust
pub struct TournamentRecord {
    pub tournament_id: u64,
    pub name: String,
    pub tournament_type: TournamentType,  // NEW
    pub max_players: u16,               // NEW (was implicit 4)
    pub entry_fee_lamports: u64,
    pub prize_pool_sol: u64,            // RENAMED for clarity
    pub prize_pool_usdc: Option<u64>,   // NEW for USDC prizes
    pub prize_mint: Option<String>,     // NEW: generic SPL token support
    pub status: TournamentStatus,
    pub players: Vec<String>,
    pub player_elos: Vec<u32>,
    pub node_ids: HashMap<String, String>,
    pub matches: Vec<TournamentMatch>,  // CHANGED: Vec instead of fixed array
    pub swiss_state: Option<SwissState>,
    pub winner: Option<String>,
    pub scheduled_at: Option<i64>,      // NEW: for scheduled tournaments
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub elo_min: u32,                   // NEW: ELO gating
    pub elo_max: u32,                   // NEW
}

// Swiss-specific state
pub struct SwissState {
    pub current_round: u8,
    pub total_rounds: u8,
    pub player_scores: HashMap<String, f64>,
    pub player_byes: HashMap<String, u8>,
    pub pairings_history: Vec<Vec<(String, String)>>,
}
```

### 1.3 Update TournamentMatch Structure
**File:** `backend/src/tournament/store.rs`

```rust
pub struct TournamentMatch {
    pub match_index: u16,        // CHANGED: u8 → u16 for larger tournaments
    pub round: u16,              // CHANGED: u8 → u16
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub winner: Option<String>,
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    pub result_source: Option<ResultSource>, // NEW: how result was determined
}

pub enum ResultSource {
    OnChain,      // Result recorded on Solana
    Oracle,       // Backend oracle submitted result
    Forfeit,      // Player didn't show/no-show
    DrawAgreed,   // Players agreed to draw
}
```

---

## Phase 2: Swiss Pairing Integration (Week 1-2)

### 2.1 Wire Swiss Pairing Crate to Backend
**File:** `backend/src/tournament/swiss_engine.rs` (NEW)

Create adapter layer between `swiss-pairing` crate and backend types:

```rust
//! Swiss pairing adapter - bridges swiss-pairing crate with backend tournament state

use swiss_pairing::{SwissPlayer, generate_pairings, calculate_standings};
use crate::tournament::store::{TournamentRecord, TournamentMatch, MatchStatus};

pub fn generate_swiss_round(
    tournament: &mut TournamentRecord,
    round: u8,
) -> Result<Vec<TournamentMatch>, SwissError> {
    let swiss_state = tournament.swiss_state.as_ref()
        .ok_or(SwissError::NotSwissTournament)?;
    
    // Convert backend players to swiss-pairing types
    let swiss_players: Vec<SwissPlayer> = tournament.players.iter()
        .enumerate()
        .map(|(idx, pubkey)| {
            let elo = tournament.player_elos.get(idx).copied().unwrap_or(1200);
            let score = swiss_state.player_scores.get(pubkey).copied().unwrap_or(0.0);
            let byes = swiss_state.player_byes.get(pubkey).copied().unwrap_or(0);
            
            SwissPlayer {
                id: pubkey.clone(),
                rating: elo,
                score,
                color_history: extract_color_history(pubkey, &tournament.matches),
                opponents: extract_opponents(pubkey, &tournament.matches),
                bye_count: byes,
                float_status: swiss_pairing::FloatStatus::None,
            }
        })
        .collect();
    
    let swiss_round = generate_pairings(round, &swiss_players, swiss_state.total_rounds)?;
    
    // Convert back to TournamentMatch
    let matches: Vec<TournamentMatch> = swiss_round.pairings.iter()
        .enumerate()
        .map(|(idx, pairing)| TournamentMatch {
            match_index: (tournament.matches.len() + idx) as u16,
            round,
            player_white: Some(pairing.white.clone()),
            player_black: Some(pairing.black.clone()),
            winner: None,
            game_id: None,
            status: MatchStatus::Pending,
            result_source: None,
        })
        .collect();
    
    Ok(matches)
}
```

### 2.2 Update join_tournament Handler
**File:** `backend/src/tournament/routes.rs`

Replace `start_bracket()` with format-aware initialization:

```rust
/// POST /tournament/:id/join
pub async fn join_tournament(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = body.get("player").and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let elo = body.get("elo").and_then(|v| v.as_u64())
        .unwrap_or(1200) as u32;
    
    let mut slot = None;
    let ok = store.update(id, |t| {
        // NEW: Check ELO gates
        if elo < t.elo_min || elo > t.elo_max {
            return; // Will fail with conflict below
        }
        
        if t.players.len() >= t.max_players as usize {
            return;
        }
        if t.players.iter().any(|p| p == player) {
            slot = Some(t.players.len());
            return;
        }
        slot = Some(t.players.len());
        t.players.push(player.to_string());
        t.player_elos.push(elo);
        t.prize_pool_sol += t.entry_fee_lamports;
        
        // NEW: Format-aware auto-start
        if should_auto_start(t) {
            initialize_tournament_format(t);
        }
    }).await;
    
    // ... rest of handler
}

fn should_auto_start(t: &TournamentRecord) -> bool {
    match &t.tournament_type {
        TournamentType::SingleElimination => t.players.len() >= t.max_players.min(4) as usize,
        TournamentType::Swiss { .. } => t.players.len() >= t.max_players as usize,
    }
}

fn initialize_tournament_format(t: &mut TournamentRecord) {
    match &t.tournament_type {
        TournamentType::SingleElimination => start_single_elimination(t),
        TournamentType::Swiss { rounds } => start_swiss(t, *rounds),
    }
}

fn start_swiss(t: &mut TournamentRecord, total_rounds: u8) {
    use crate::tournament::swiss_engine::generate_swiss_round;
    
    t.swiss_state = Some(SwissState {
        current_round: 1,
        total_rounds,
        player_scores: HashMap::new(),
        player_byes: HashMap::new(),
        pairings_history: Vec::new(),
    });
    
    // Generate first round pairings
    let first_round = generate_swiss_round(t, 1)
        .expect("Valid Swiss initialization");
    t.matches.extend(first_round);
    t.status = TournamentStatus::Active;
    t.started_at = Some(chrono::Utc::now().timestamp());
}
```

### 2.3 Add Swiss Round Advancement Endpoint
**File:** `backend/src/tournament/routes.rs`

```rust
/// POST /admin/tournament/:id/advance-round
pub async fn advance_swiss_round(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::tournament::swiss_engine::generate_swiss_round;
    
    let ok = store.update(id, |t| {
        let swiss_state = t.swiss_state.as_mut()
            .ok_or(StatusCode::BAD_REQUEST)?;
        
        if swiss_state.current_round >= swiss_state.total_rounds {
            // Tournament complete
            t.status = TournamentStatus::Completed;
            t.completed_at = Some(chrono::Utc::now().timestamp());
            return Ok(());
        }
        
        swiss_state.current_round += 1;
        let next_round = generate_swiss_round(t, swiss_state.current_round)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        t.matches.extend(next_round);
        
        Ok(())
    }).await;
    
    // ... response handling
}
```

---

---

## Phase 4: Scheduled Tournaments & Background Tasks (Week 2-3)

### 4.1 Add Tournament Scheduler
**File:** `backend/src/tasks/tournament_scheduler.rs` (NEW)

```rust
//! Background task for scheduled tournament auto-start

use tokio::time::{interval, Duration};
use crate::signing::TournamentStore;
use crate::tournament::store::TournamentStatus;

pub fn spawn_tournament_scheduler(store: TournamentStore) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30)); // Check every 30s
        
        loop {
            ticker.tick().await;
            
            let tournaments = store.list().await;
            let now = chrono::Utc::now().timestamp();
            
            for t in tournaments {
                if t.status != TournamentStatus::Registration {
                    continue;
                }
                
                if let Some(scheduled_at) = t.scheduled_at {
                    if now >= scheduled_at && t.players.len() >= t.min_players as usize {
                        // Auto-start the tournament
                        let _ = store.update(t.tournament_id, |record| {
                            initialize_tournament_format(record);
                        }).await;
                        
                        tracing::info!(
                            "Auto-started scheduled tournament {} at {}",
                            t.tournament_id,
                            chrono::Utc::now()
                        );
                    } else if now >= scheduled_at && t.players.len() < t.min_players as usize {
                        // Cancel due to insufficient players
                        let _ = store.update(t.tournament_id, |record| {
                            record.status = TournamentStatus::Cancelled;
                        }).await;
                    }
                }
            }
        }
    });
}
```

### 4.2 Update Tasks Module
**File:** `backend/src/tasks/mod.rs`

```rust
//! Background tasks for the XFChess backend.

pub mod matchmaking;
pub mod fee_claimer;
pub mod tournament_scheduler;  // NEW
```

### 4.3 Update Create Tournament Endpoint
**File:** `backend/src/tournament/routes.rs`

```rust
#[derive(Deserialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    pub tournament_type: TournamentType,
    pub max_players: u16,
    pub min_players: u16,           // NEW: minimum to start
    pub entry_fee_lamports: u64,
    pub prize_mint: Option<String>, // NEW: SPL token mint (None = SOL)
    pub scheduled_at: Option<i64>,  // NEW: Unix timestamp to auto-start
    pub elo_min: Option<u32>,       // NEW
    pub elo_max: Option<u32>,       // NEW
    // Swiss-specific
    pub swiss_rounds: Option<u8>,
    // Arena-specific  
    pub arena_duration_minutes: Option<u16>,
    pub arena_increment_seconds: Option<u8>,
}
```

---

## Phase 5: Spectator System (Week 3)

### 5.1 Add Spectator Node Role
**File:** `src/multiplayer/network_protocol.rs` (or create if needed)

```rust
/// Peer role in the gossip network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Player,      // Active game participant
    Spectator,   // Read-only observer
    Relay,       // TURN relay node
    Arbiter,     // Tournament official/oracle
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub wallet_pubkey: String,
    pub role: NodeRole,
    pub connected_game: Option<u64>,
    pub subscribe_topics: Vec<String>, // e.g., ["/xfchess-game/123", "/xfchess-tournament/456"]
}
```

### 5.2 Spectator Feed System
**File:** `src/multiplayer/spectator_feed.rs` (NEW)

```rust
//! Spectator feed system - broadcasts game state to non-participants

use crate::multiplayer::{NetworkMessage, NodeRole, BraidNetworkState};

/// System that filters and broadcasts game state to spectators
pub fn feed_local_moves_to_rollup(
    network_state: Res<BraidNetworkState>,
    game_state: Res<MultiplayerGameState>,
) {
    let Some(game_id) = game_state.game_id else { return };
    let Some(tx) = &network_state.message_sender else { return };
    
    // Create spectator-safe message (no sensitive data)
    let spectator_msg = NetworkMessage::GameStateBroadcast {
        game_id,
        fen: game_state.current_fen.clone(),
        last_move: game_state.last_move.clone(),
        white_time_ms: game_state.white_time_ms,
        black_time_ms: game_state.black_time_ms,
        move_number: game_state.move_number,
        is_check: game_state.is_check,
        // DELIBERATELY EXCLUDED: player wallets, private game data
    };
    
    // Broadcast to /xfchess-game/{game_id} topic
    // Gossip system handles routing; spectators subscribe but don't emit
    let _ = tx.send(spectator_msg);
}

/// System to handle spectator subscriptions
pub fn handle_spectator_subscriptions(
    mut network_events: EventReader<NetworkEvent>,
    mut spectator_registry: ResMut<SpectatorRegistry>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::PeerConnected { peer_info } if peer_info.role == NodeRole::Spectator => {
                for topic in &peer_info.subscribe_topics {
                    spectator_registry.add_spectator(topic, peer_info.node_id.clone());
                }
            }
            NetworkEvent::PeerDisconnected { node_id } => {
                spectator_registry.remove_spectator(node_id);
            }
            _ => {}
        }
    }
}
```

### 5.3 Compression for Move Broadcasts
**File:** `src/multiplayer/compression.rs` (NEW)

```rust
//! Network message compression using bincode + zstd

use bincode;
use zstd::bulk::{compress, decompress};

const COMPRESSION_LEVEL: i32 = 3; // Balance speed vs ratio

/// Compress a NetworkMessage for gossip broadcast
pub fn compress_message(msg: &NetworkMessage) -> Result<Vec<u8>, CompressionError> {
    // Serialize with bincode (compact binary format)
    let serialized = bincode::serialize(msg)?;
    
    // Compress with zstd (~5x smaller than JSON for typical chess messages)
    let compressed = compress(&serialized, COMPRESSION_LEVEL)?;
    
    Ok(compressed)
}

/// Decompress a received message
pub fn decompress_message(data: &[u8]) -> Result<NetworkMessage, CompressionError> {
    let decompressed = decompress(data, 1024 * 1024)?; // 1MB limit
    let msg = bincode::deserialize(&decompressed)?;
    Ok(msg)
}

#[derive(Debug)]
pub enum CompressionError {
    Serialization(bincode::Error),
    Compression(zstd::stream::zstd_sys::ZSTD_ErrorCode),
    SizeLimit,
}
```

---

## Phase 6: Reconnect/Resume with SQLite (Week 3)

### 6.1 SQLite Session Persistence
**File:** `backend/src/db/sessions.rs` (NEW)

```rust
//! Persistent session storage for disconnect recovery

use rusqlite::{params, Connection, OptionalExtension};
use serde_json;

pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    pub fn new(db_path: &str) -> Result<Self, DbError> {
        let conn = Connection::open(db_path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS active_sessions (
                session_id TEXT PRIMARY KEY,
                game_id INTEGER NOT NULL,
                player_white TEXT NOT NULL,
                player_black TEXT NOT NULL,
                current_fen TEXT NOT NULL,
                move_history TEXT NOT NULL,  -- JSON array
                white_time_ms INTEGER,
                black_time_ms INTEGER,
                last_activity INTEGER NOT NULL,  -- Unix timestamp
                grace_period_ends INTEGER,       -- When session can be reclaimed
                status TEXT NOT NULL  -- 'active', 'paused', 'resumable'
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_game ON active_sessions(game_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_player ON active_sessions(player_white, player_black)",
            [],
        )?;
        
        Ok(Self { conn })
    }
    
    pub fn save_session(&self, session: &ActiveSession) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO active_sessions 
             (session_id, game_id, player_white, player_black, current_fen, 
              move_history, white_time_ms, black_time_ms, last_activity, 
              grace_period_ends, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                session.session_id,
                session.game_id,
                session.player_white,
                session.player_black,
                session.current_fen,
                serde_json::to_string(&session.move_history)?,
                session.white_time_ms,
                session.black_time_ms,
                session.last_activity,
                session.grace_period_ends,
                session.status.to_string(),
            ],
        )?;
        Ok(())
    }
    
    pub fn get_resumable_session(
        &self,
        player_pubkey: &str,
    ) -> Result<Option<ActiveSession>, DbError> {
        let now = chrono::Utc::now().timestamp();
        
        let mut stmt = self.conn.prepare(
            "SELECT * FROM active_sessions 
             WHERE (player_white = ?1 OR player_black = ?1)
             AND status = 'resumable'
             AND grace_period_ends > ?2
             ORDER BY last_activity DESC
             LIMIT 1"
        )?;
        
        // ... deserialize and return
    }
}

pub struct ActiveSession {
    pub session_id: String,  // UUID v4
    pub game_id: u64,
    pub player_white: String,
    pub player_black: String,
    pub current_fen: String,
    pub move_history: Vec<String>,
    pub white_time_ms: i64,
    pub black_time_ms: i64,
    pub last_activity: i64,
    pub grace_period_ends: i64,  // e.g., 5 minutes after disconnect
    pub status: SessionStatus,
}

pub enum SessionStatus {
    Active,
    Paused,     // Temporarily disconnected
    Resumable,  // Can rejoin within grace period
    Expired,    // Grace period ended
}
```

### 6.2 Update MultiplayerGameState
**File:** `src/multiplayer/state.rs`

```rust
#[derive(Resource)]
pub struct MultiplayerGameState {
    // ... existing fields ...
    
    // NEW: Persistence fields
    pub session_id: Option<String>,
    pub last_persisted: Option<Instant>,
    pub disconnect_detected_at: Option<Instant>,
}
```

---

## Phase 7: TURN Relay for NAT Traversal (Week 4)

### 7.1 TURN Client Integration
**File:** `src/multiplayer/turn_relay.rs` (NEW)

```rust
//! TURN relay for symmetric NAT traversal
//! Falls back when iroh hole-punching fails

use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct TurnRelayConfig {
    pub server_addr: SocketAddr,
    pub username: String,
    pub password: String,
    pub realm: String,
}

pub struct TurnRelayClient {
    socket: UdpSocket,
    config: TurnRelayConfig,
    relayed_addr: Option<SocketAddr>,
    allocation_lifetime: Duration,
}

impl TurnRelayClient {
    pub async fn connect(config: TurnRelayConfig) -> Result<Self, TurnError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        
        // TURN allocation request
        let mut client = Self {
            socket,
            config,
            relayed_addr: None,
            allocation_lifetime: Duration::from_secs(600),
        };
        
        client.allocate().await?;
        Ok(client)
    }
    
    async fn allocate(&mut self) -> Result<(), TurnError> {
        // STUN/TURN allocation request
        // ... implement rfc5766
    }
    
    pub async fn relay_packet(&self, data: &[u8], peer: SocketAddr) -> Result<(), TurnError> {
        // Send data through TURN relay
        // Format: ChannelData message or Send indication
    }
}

/// Detect if we need TURN relay
pub async fn detect_nat_type() -> NatType {
    // Try iroh hole-punch first
    if can_hole_punch().await {
        return NatType::OpenOrFullCone;
    }
    
    // Check for symmetric NAT
    if is_symmetric_nat().await {
        return NatType::Symmetric;
    }
    
    NatType::RestrictedCone
}

pub enum NatType {
    OpenOrFullCone,  // Direct connection possible
    RestrictedCone,  // May need STUN but not TURN
    Symmetric,       // Requires TURN relay
}
```

### 7.2 Integration with VPS Client
**File:** `src/multiplayer/rollup/vps_client.rs`

```rust
pub async fn connect_with_fallback(
    vps_addr: &str,
    turn_config: Option<TurnRelayConfig>,
) -> Result<Connection, ConnectionError> {
    // Try direct connection first
    match connect_direct(vps_addr).await {
        Ok(conn) => return Ok(conn),
        Err(e) if is_nat_error(&e) => {
            tracing::warn!("Direct connection failed due to NAT, trying TURN relay");
        }
        Err(e) => return Err(e),
    }
    
    // Fall back to TURN relay
    if let Some(config) = turn_config {
        let relay = TurnRelayClient::connect(config).await?;
        connect_via_relay(relay, vps_addr).await
    } else {
        Err(ConnectionError::NatTraversalFailed)
    }
}
```

---

## Phase 8: Multi-Token Prize Pools & Streaming Payouts (Week 4-5)

### 8.1 Extend On-Chain Tournament State
**File:** `programs/xfchess-game/src/state/tournament.rs`

```rust
#[account]
#[derive(InitSpace)]
pub struct Tournament {
    // ... existing fields ...
    
    /// Prize token mint (None = wrapped SOL)
    pub prize_token_mint: Option<Pubkey>,
    
    /// USDC prize vault (legacy, prefer prize_token_mint)
    pub usdc_prize_vault: Option<Pubkey>,
    
    /// Prize distribution method
    pub payout_type: PayoutType,
    
    /// Streaming vesting parameters (if applicable)
    pub vesting_params: Option<VestingParams>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum PayoutType {
    LumpSum,           // Immediate full payout
    StreamingLinear,   // Linear vesting over N days
    StreamingCliff,    // Cliff vesting (e.g., 50% at 30 days, 50% at 60 days)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub struct VestingParams {
    pub start_time: i64,
    pub duration_seconds: i64,     // e.g., 7 days = 604800
    pub cliff_seconds: Option<i64>, // Optional cliff
}
```

### 8.2 SPL Token Prize Pool Funding
**File:** `programs/xfchess-game/src/tournament_ix/fund_prize.rs`

```rust
//! Generic SPL token prize pool funding (was USDC-specific)

#[derive(Accounts)]
#[instruction(tournament_id: u64, amount: u64)]
pub struct FundTokenPrize<'info> {
    #[account(mut, seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()], bump)]
    pub tournament: Account<'info, Tournament>,
    
    #[account(
        mut,
        constraint = prize_vault.mint == tournament.prize_token_mint.unwrap() 
            @ GameErrorCode::InvalidTokenMint
    )]
    pub prize_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = funder_token_account.mint == tournament.prize_token_mint.unwrap()
    )]
    pub funder_token_account: Account<'info, TokenAccount>,
    
    pub funder: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<FundTokenPrize>, _tournament_id: u64, amount: u64) -> Result<()> {
    // Transfer SPL tokens to prize vault
    anchor_spl::token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.funder_token_account.to_account_info(),
                to: ctx.accounts.prize_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        ),
        amount,
    )?;
    
    ctx.accounts.tournament.prize_pool += amount;
    Ok(())
}
```

### 8.3 Streaming Payout via Streamflow (or Simple Vesting)
**File:** `programs/xfchess-game/src/tournament_ix/claim_streaming.rs` (NEW)

Option A: Simple on-chain vesting (no external dependencies):

```rust
//! Simple linear vesting for tournament prizes

#[derive(Accounts)]
pub struct ClaimStreamingPrize<'info> {
    #[account(mut, seeds = [TOURNAMENT_SEED, &tournament.tournament_id.to_le_bytes()], bump)]
    pub tournament: Account<'info, Tournament>,
    
    #[account(mut)]
    pub prize_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
pub winner_token_account: Account<'info, TokenAccount>,
    
    /// CHECK: Winner pubkey verified against tournament.winner
    #[account(address = tournament.winner.unwrap())]
    pub winner: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<ClaimStreamingPrize>) -> Result<()> {
    let t = &ctx.accounts.tournament;
    let vesting = t.vesting_params.as_ref()
        .ok_or(GameErrorCode::NoVestingConfigured)?;
    
    let now = Clock::get()?.unix_timestamp;
    let elapsed = now.saturating_sub(vesting.start_time);
    
    // Calculate vested amount
    let total_prize = t.prize_pool;
    let vested_ratio = (elapsed as f64 / vesting.duration_seconds as f64)
        .min(1.0);
    let vested_amount = (total_prize as f64 * vested_ratio) as u64;
    
    // Check already_claimed tracking (would need additional state)
    let claimable = vested_amount.saturating_sub(t.already_claimed);
    
    require!(claimable > 0, GameErrorCode::NothingToClaim);
    
    // Transfer claimable amount
    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.prize_vault.to_account_info(),
                to: ctx.accounts.winner_token_account.to_account_info(),
                authority: ctx.accounts.tournament.to_account_info(),
            },
            &[&[TOURNAMENT_SEED, &t.tournament_id.to_le_bytes(), &[t.bump]]],
        ),
        claimable,
    )?;
    
    t.already_claimed += claimable;
    Ok(())
}
```

Option B: Streamflow integration (requires their SDK):
- Use Streamflow SDK to create vesting contract on prize pool funding
- Tournament winner is set as beneficiary
- Eliminates need for custom vesting logic

---

## Phase 9: ELO Gating & Final Integration (Week 6)

### 9.1 ELO Gate Enforcement
**File:** `backend/src/tournament/routes.rs`

Already sketched in join_tournament above - add explicit error:

```rust
#[derive(Deserialize)]
pub struct JoinTournamentReq {
    pub player: String,
    pub elo: u32,
    pub signed_elo: String,  // Optional: ELO attestation from oracle
}

pub async fn join_tournament(
    // ...
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Verify ELO attestation if provided
    if let Some(signed) = req.signed_elo {
        if !verify_elo_attestation(&req.player, req.elo, &signed) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    
    // Check gates
    let ok = store.update(id, |t| {
        if req.elo < t.elo_min || req.elo > t.elo_max {
            return Err(TournamentError::EloOutOfRange);
        }
        // ... rest
    }).await;
}
```

### 9.2 Testing Strategy

**Unit Tests:**
- Swiss pairing correctness (FIDE compliance)
- Round-robin schedule generation
- Arena point calculations
- ELO gate enforcement

**Integration Tests:**
- Full tournament lifecycle (each format)
- Spectator feed message filtering
- SQLite session persistence/recovery
- TURN relay fallback
- Multi-token prize pools

**Load Tests:**
- 256-player Swiss tournament simulation
- 1000 concurrent spectators
- Network compression benchmarks (target: 5x reduction)

---

## Implementation Timeline Summary

| Week | Focus | Key Deliverables |
|------|-------|-----------------|
| 1 | Data Structures | TournamentType expansion, Vec<Match>, SwissState |
| 1-2 | Swiss Integration | swiss_engine.rs, advance-round endpoint |
| 2 | Braid Scheduling | tournament_scheduler.rs, SQLite schema |
| 3 | Spectators | NodeRole, compression.rs, spectator_feed.rs |
| 3 | Persistence | sessions.rs, reconnect flow |
| 4 | TURN Relay | turn_relay.rs, NAT detection |
| 4-5 | Multi-Token | SPL token prizes, streaming vesting |
| 5-6 | Integration | ELO gates, testing, documentation |

---

## Dependencies to Add

**Backend Cargo.toml:**
```toml
[dependencies]
# Already present
swiss-pairing = { path = "../crates/swiss-pairing" }

# NEW
rusqlite = { version = "0.30", features = ["bundled", "serde_json"] }
zstd = "0.13"
bincode = "1.3"
```

**Program Cargo.toml:**
```toml
[dependencies]
# NEW for streaming
streamflow-sdk = { version = "0.6", optional = true }
```

---

## Migration Notes

1. **Existing tournaments:** Will use old `[Option<TournamentMatch>; 3]` - maintain backward compatibility or migrate on read
2. **On-chain TournamentType:** Add new variants at end to maintain binary compatibility
3. **Database:** Add columns to SQLite schema incrementally with defaults

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Swiss crate integration bugs | Extensive property-based testing (pairings are valid, no repeats) |
| SQLite performance at scale | Connection pooling, WAL mode, consider Redis for hot data |
| TURN relay costs | Usage caps, only enable for symmetric NAT, self-hosted coturn |
| Streaming complexity | Start with simple linear vesting, Streamflow as v2 |

