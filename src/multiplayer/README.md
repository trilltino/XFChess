# Multiplayer Module

## Purpose
The multiplayer module handles P2P networking, wager state management, and game state synchronization between players. It enables both local P2P games and Solana-backed wager matches.

## Impact on Game
This module **powers multiplayer functionality**:
- **P2P Networking:** Direct peer-to-peer connection via Iroh
- **Move Relay:** Fast move transmission between players
- **Wager Tracking:** Monitors escrow and payout state
- **State Sync:** Ensures both players see the same board
- **Rollup Batching:** Groups moves for efficient blockchain submission (ER)

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     Multiplayer Module                        │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐    │
│   │  P2P Network │   │ Wager State  │   │  Rollup Mgr  │    │
│   │   (Iroh)     │   │  (escrow)    │   │  (batches)   │    │
│   └──────────────┘   └──────────────┘   └──────────────┘    │
│          │                  │                  │             │
│          └──────────────────┼──────────────────┘             │
│                             v                                │
│                    ┌──────────────┐                         │
│                    │ Game Session │                         │
│                    │   Manager    │                         │
│                    └──────────────┘                         │
│                             │                                │
│                             v                                │
│                    ┌──────────────┐                         │
│                    │   Bevy ECS   │                         │
│                    │   Events     │                         │
│                    └──────────────┘                         │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

## Key Components

### P2P Network (`mod.rs`)
Iroh-based gossip protocol for move transmission:
```rust
pub struct MultiplayerState {
    pub node_id: Option<String>,
    pub peers: HashMap<String, PeerInfo>,
    pub game_session: Option<GameSession>,
}
```

**Network Events:**
- `NetworkInitialized` - P2P node spawned
- `MoveReceived` - Opponent sent move
- `PeerConnected` - Direct connection established

### Wager State (`wager_state.rs`)
Tracks on-chain wager information:
```rust
pub struct WagerState {
    pub game_id: Option<u64>,
    pub wager_per_player: Option<f64>,
    pub total_pot: Option<f64>,
    pub your_pubkey: Option<String>,
    pub opponent_pubkey: Option<String>,
    pub status: WagerMatchStatus,
}
```

**States:**
- `Idle` - No active wager
- `WaitingForOpponent` - Created game, waiting
- `Matched` - Both players joined
- `InProgress` - Game active
- `Completed` - Game finished

### Rollup Manager (`rollup_manager.rs`)
Batches moves for MagicBlock ER submission:
```rust
pub struct EphemeralRollupManager {
    pub game_id: u64,
    pub pending_moves: Vec<MoveRecord>,
    pub batch_threshold: usize, // e.g., 5 moves
}
```

**Why batching?**
- Reduces on-chain transactions
- Lowers gas costs
- Enables sub-second moves on ER

### Game Session
Active match tracking:
```rust
pub struct GameSession {
    pub session_id: String,
    pub opponent: PeerInfo,
    pub stake_amount: f64,
    pub game_state: MultiplayerGameState,
}
```

## Game Flow

### 1. P2P Connection
```
Player 1 (Host)          Player 2 (Joiner)
     │                          │
     │-- spawn_node() ---------->│
     │   node_id = abc123        │
     │                          │
     │<-- bootstrap_node() ------│
     │   connects to abc123      │
     │                          │
     │-- gossip: game_invite --->│
     │                          │
     │<-- gossip: invite_accepted│
```

### 2. Move Transmission
```
Player moves piece
      │
      v
Local validation (legal move?)
      │
      v
Send via P2P gossip
      │
      v
Opponent receives
      │
      v
Opponent validates
      │
      v
Update opponent's board
```

### 3. Wager Flow (with Solana)
```
Create Game              Join Game
     │                        │
     v                        v
Escrow: 0.01 SOL      Escrow: 0.01 SOL
     │                        │
     └────────┬───────────────┘
              │
              v
         Total Pot: 0.02 SOL
              │
              v
        Game Complete
              │
              v
    Winner receives 0.02 SOL
```

## Configuration

### P2P Settings
```rust
const DEFAULT_P2P_PORT: u16 = 5001;
const GAME_TOPIC: &str = "xfchess_game_v1";
```

### Rollup Settings
```rust
const BATCH_SIZE: usize = 5;      // Moves per batch
const BATCH_TIMEOUT: u64 = 30000; // 30 seconds max wait
```

## Usage

### Initialize Multiplayer
```rust
// In Bevy app setup
app.add_plugins(MultiplayerPlugin)
   .insert_resource(MultiplayerState::default());
```

### Host Game
```rust
fn host_game(
    mut multiplayer: ResMut<MultiplayerState>,
) {
    multiplayer.host_game();
    // P2P node spawned, waiting for connections
}
```

### Join Game
```rust
fn join_game(
    mut multiplayer: ResMut<MultiplayerState>,
    host_node_id: String,
) {
    multiplayer.connect_to_peer(host_node_id);
}
```

### Send Move
```rust
fn send_move(
    multiplayer: Res<MultiplayerState>,
    move_uci: String,
) {
    let msg = NetworkMessage::ChessMove {
        uci: move_uci,
        timestamp: now(),
    };
    multiplayer.send_message(msg);
}
```

## Events

### NetworkEvent
```rust
pub enum NetworkEvent {
    NetworkInitialized { node_id: String, ... },
    MoveReceived(String, String), // (peer_id, move_uci)
    PeerConnected(String),
    PeerDisconnected(String),
    GameInviteReceived(String, GamePreferences),
}
```

### RollupEvent
```rust
pub enum RollupEvent {
    BatchReady { game_id, moves, next_fens },
    BatchCommitted { signature },
    BatchFailed { error },
}
```

## Testing

### Local P2P Test
```bash
# Terminal 1 - Host
cargo run -- --p2p-port 5001

# Terminal 2 - Join
cargo run -- --p2p-port 5002 --bootstrap-node <host_id>
```

### Wager Game Test
```bash
# Launch with Solana session
cargo run -- --session-config session.json
```

## Troubleshooting

### Can't connect to peer
- Check firewall settings (ports 5001-5010)
- Verify node IDs match
- Try restarting both clients

### Moves not syncing
- Check P2P connection status
- Verify both on same game topic
- Check move validation errors

### Wager state wrong
- Sync with Solana: `fetch_game_state()`
- Check wallet connection
- Verify game PDA address

## Dependencies

- `iroh` - P2P networking
- `braid-iroh` - CRDT sync over Iroh
- `tokio` - Async runtime
- `serde` - Message serialization

## Performance

- **P2P Latency:** <50ms local, <200ms global
- **Gossip Propagation:** <500ms for 2 peers
- **Batch Commit:** ~2-5 seconds on Solana

## Security

- **Move Validation:** Always validate opponent moves
- **State Verification:** Cross-check with Solana
- **Replay Protection:** Include timestamps and nonces
