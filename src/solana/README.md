# Solana Integration Module

## Purpose
The Solana module provides blockchain connectivity for the XFChess native game client. It handles wallet session management, move submission, and game state synchronization with the Solana devnet/mainnet.

## Impact on Game
This module **enables wager-based gameplay**:
- **Session Management:** Secure ephemeral keys for signing transactions
- **Move Recording:** Submits chess moves to the on-chain program
- **State Sync:** Fetches current game state from blockchain
- **Payout Claims:** Handles winner payment distribution

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    Solana Module                            │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │   Session    │───>│   Client     │───>│   Program    │ │
│  │  Management  │    │   (RPC)      │    │   Interface  │ │
│  └──────────────┘    └──────────────┘    └──────────────┘ │
│         │                   │                   │          │
│         v                   v                   v          │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │  GameSession │    │  SolanaGame  │    │ Instructions │ │
│  │   (ephemeral │    │    Sync      │    │  (Anchor IDL)│ │
│  │     keys)    │    │              │    │              │ │
│  └──────────────┘    └──────────────┘    └──────────────┘ │
│                                                             │
└────────────────────────────────────────────────────────────┘
                            │
                            v
              ┌─────────────────────────┐
              │   Solana Devnet/Mainnet  │
              │   Program: xfchess_game  │
              └─────────────────────────┘
```

## Key Components

### Session Management (`session.rs`)
Manages ephemeral keypairs for game signing:
```rust
pub struct GameSession {
    pub wallet_pubkey: Pubkey,
    pub session_signer: Keypair,
    pub session_token_pda: Pubkey,
    pub expires_at: i64,
}
```

**Why ephemeral keys?**
- Security: Main wallet private key never leaves browser
- Speed: No wallet popup for every move
- Delegation: Can delegate to MagicBlock ER validator

### Solana Client (`client.rs`)
RPC client for blockchain interaction:
- `fetch_game_state()` - Get current game account
- `submit_move()` - Record move on-chain
- `claim_payout()` - Winner claims pot

### Program Interface (`instructions.rs`)
Anchor IDL-based instruction builders:
- `create_game_ix`
- `join_game_ix`
- `record_move_ix`
- `finalize_game_ix`

### Game Sync (`mod.rs`)
Bevy systems for blockchain synchronization:
- Polls game state every 5 seconds
- Detects opponent moves
- Updates local board

## Flow: Submitting a Move

```
1. Player moves piece in UI
        │
        v
2. Local move validation (shakmaty)
        │
        v
3. Generate move UCI (e.g., "e2e4")
        │
        v
4. Create transaction
   - Game PDA
   - Session signer
   - Move data
        │
        v
5. Submit to Solana
        │
        v
6. On-chain validation
   - Is it player's turn?
   - Valid chess move?
   - Game active?
        │
        v
7. Transaction confirmed
   - Move recorded
   - Turn switches
   - Opponent notified
```

## Configuration

### Session Parameters
Loaded from session JSON:
```json
{
  "sessionKey": "base58_secret_key",
  "sessionPubkey": "base58_public_key",
  "rpcUrl": "https://api.devnet.solana.com",
  "gamePda": "GamePDA_address"
}
```

### Environment Variables
```rust
const PROGRAM_ID: &str = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP";
const DEFAULT_RPC: &str = "https://api.devnet.solana.com";
```

## Usage

### Initialize from Session Config
```rust
// In main.rs - CLI loads session config
let mut cli = Cli::parse();
cli.load_session_config()?;

// GameConfig populated with Solana params
let game_config = GameConfig {
    session_key: cli.session_key,
    session_pubkey: cli.session_pubkey,
    game_pda: cli.game_pda,
    rpc_url: cli.rpc_url,
    // ...
};
```

### Submit Move
```rust
// In game system
fn submit_move_system(
    solana_client: Res<SolanaClient>,
    session: Res<GameSession>,
) {
    let move_uci = "e2e4";
    let signature = solana_client
        .submit_move(session, move_uci)
        .await?;
    
    println!("Move recorded: {}", signature);
}
```

### Fetch Game State
```rust
fn sync_game_state(
    solana_client: Res<SolanaClient>,
    game_pda: Res<GamePDA>,
) {
    let game_account = solana_client
        .fetch_game(game_pda.0)
        .await?;
    
    // Update local board with opponent moves
    update_board(game_account.fen);
}
```

## Testing

### Local Validator
```bash
solana-test-validator
```

### Devnet Testing
```bash
cargo run -- --session-config test_session.json
```

## Troubleshooting

### "Insufficient funds"
- Get devnet SOL: https://faucet.solana.com/

### "Invalid session"
- Session may have expired
- Regenerate via web UI

### "Program not found"
- Check RPC URL (devnet vs mainnet)
- Verify program ID

## Dependencies

- `solana-sdk` - Core types
- `solana-client` - RPC client
- `solana-program` - Program CPI
- `anchor-lang` - IDL handling

## Security Notes

- **Never** log session private keys
- **Always** validate on-chain state
- **Use** devnet for testing
- **Rotate** session keys regularly
