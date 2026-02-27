# Board State Synchronization Plan

## Overview
Implement versioned board state synchronization for P2P chess using Braid's Simpleton merge-type and text-based state sharing.

## Architecture

### 1. State Representation
```rust
// Board state as FEN + metadata
struct ChessBoardState {
    fen: String,                    // e.g., "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    move_counter: u32,              // Total moves made
    last_move: Option<MoveRecord>,  // Last move for replay
    captured_pieces: CapturedPieces,// Track captures
    hash: [u8; 32],                 // SHA-256 of FEN + move_counter
}

// Serialized format for sharing
// FEN|move_counter|from_x,from_y,to_x,to_y|hash
// Example: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR|1|4,1,4,3|a3f2b8..."
```

### 2. Braid Integration Using Simpleton

The `SimpletonMergeType` in `crates/braid-core/src/core/merge/simpleton.rs` provides:
- Text-based CRDT merging
- Version tracking via `char_counter` and `peer_id`
- Automatic conflict resolution for concurrent edits

**Implementation:**
```rust
// New resource for board state sync
#[derive(Resource)]
pub struct BoardStateSync {
    pub merge_type: SimpletonMergeType,
    pub last_known_state: String,
    pub pending_moves: Vec<MoveRecord>,
}

impl BoardStateSync {
    pub fn new(peer_id: &str) -> Self {
        Self {
            merge_type: SimpletonMergeType::new(peer_id),
            last_known_state: String::new(),
            pending_moves: Vec::new(),
        }
    }
    
    pub fn serialize_state(&self, engine: &ChessEngine) -> String {
        // Format: FEN|move_counter|last_move|captured_white|captured_black
        let fen = engine.to_fen();
        // ... serialize to string
        format!("{}|{}|...", fen, move_counter)
    }
    
    pub fn apply_remote_state(&mut self, state_str: &str) -> Result<BoardDiff, SyncError> {
        // Parse received state
        // Calculate diff from current state
        // Return moves to apply
    }
}
```

### 3. Sync Protocol

**File-based sharing (as suggested):**
- Write board state to `board_state.txt` in braid sync folder
- Braid automatically syncs file between peers
- Simpleton handles merge conflicts

**Or direct braid message:**
- Use braid-http protocol to broadcast state changes
- Subscribe to state updates from peers

### 4. Conflict Resolution

**Version Vector Approach:**
```rust
struct BoardVersion {
    peer_id: String,
    move_counter: u32,
    timestamp: u64,
}

// When receiving conflicting states:
// 1. Compare move_counters - higher wins
// 2. If equal, compare timestamps - later wins
// 3. If still equal, lexicographic peer_id wins (deterministic)
```

**Move Replay:**
If states diverge, replay moves from common ancestor:
```rust
fn reconcile_states(local: &BoardState, remote: &BoardState) -> BoardState {
    // Find common ancestor
    // Replay moves from both sides
    // Apply deterministic tie-breaker
}
```

### 5. Implementation Steps

1. **Create `BoardStateSync` resource** in `src/game/sync/`:
   - Wrap SimpletonMergeType
   - Handle serialization/deserialization
   - Track pending moves

2. **Add state change detection**:
   - After each move, serialize board state
   - Write to braid-synced file or broadcast

3. **Implement state receiver**:
   - Watch for file changes or braid messages
   - Parse received state
   - Validate against local state
   - Apply if valid and newer

4. **Add validation**:
   - Verify FEN is valid
   - Verify move is legal from previous state
   - Verify hash matches content

5. **UI Integration**:
   - Show sync status (connected, syncing, conflict)
   - Display opponent's last move
   - Show network latency

### 6. File Structure

```
src/game/sync/
├── mod.rs                    # Existing sync module
├── board_state_sync.rs       # NEW: BoardStateSync resource
├── state_validator.rs        # NEW: Validate received states
├── conflict_resolver.rs      # NEW: Resolve state conflicts
└── braid_file_sync.rs        # NEW: File-based sync using braid-blob
```

### 7. Integration Points

**With existing game systems:**
- Hook into `execute_move()` in `shared.rs` - serialize after each move
- Hook into `handle_network_moves()` in `network_move.rs` - receive via braid
- Update `GameSyncPlugin` to initialize BoardStateSync

**With braid crates:**
- Use `braid-blob` for file-based sync
- Use `braid-core` Simpleton for conflict resolution
- Use `braid-iroh` for P2P transport

### 8. Example Flow

```
1. White moves pawn e2->e4
2. Game system executes move locally
3. BoardStateSync serializes new state: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR|1|..."
4. Writes to braid-synced board_state.txt
5. Braid syncs to Black's client
6. Black's client receives state update
7. Validates state (legal move, correct hash)
8. Applies to local engine
9. Updates board visually
```

### 9. Error Handling

- **Invalid FEN**: Reject state, request resync
- **Illegal move**: Reject state, stay at current state
- **Hash mismatch**: Reject state, potential tampering
- **Stale state**: Ignore if move_counter < local
- **Future state**: Buffer if move_counter > local + 1

### 10. Testing Strategy

- Unit tests for serialization/deserialization
- Unit tests for conflict resolution
- Integration tests with two local clients
- Network simulation (latency, packet loss)
- Stress test (rapid moves from both sides)

## Benefits of This Approach

1. **Simple**: Text-based, human-readable state
2. **Proven**: Uses braid's tested Simpleton CRDT
3. **File-based**: Easy to debug, can inspect state file
4. **Automatic merging**: Braid handles concurrent edits
5. **Deterministic**: Clear rules for conflict resolution
6. **Versioned**: Move counter prevents stale state application

## Next Steps

1. Review braid-blob crate for file sync capabilities
2. Implement BoardStateSync resource
3. Add serialization to FEN format
4. Integrate with existing move execution
5. Test with two-node setup
