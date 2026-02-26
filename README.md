# XFChess

A high-performance chess game built with Rust, featuring both single-player and multiplayer modes, with Solana blockchain integration for wager-based gameplay.

## Overview

XFChess is a modern chess application that combines a high-performance game engine with peer-to-peer networking and blockchain technology. The project is built using the Rust programming language and leverages the Bevy game engine for rendering and game logic.

## Architecture

The project follows a modular architecture with clear separation of concerns:

- **Core Engine**: Chess move validation, game state management, and AI opponents
- **Networking**: Peer-to-peer multiplayer using Iroh/Braid protocols
- **Blockchain**: Solana integration for wagering and game state anchoring
- **Presentation**: 3D rendering and UI built with Bevy

## Solana Integration

### Program Details

The Solana program (`xfchess-game`) is deployed on devnet and provides on-chain functionality for wager-based chess games.

**Program ID**: `3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP`

**Network**: Devnet (https://api.devnet.solana.com)

### Instructions

The program supports the following instructions:

1. **InitProfile**: Initialize a player profile with ELO rating tracking
   - Creates a player profile PDA (Program Derived Address)
   - Initializes ELO rating at 1200
   - Tracks wins, losses, and games played

2. **CreateGame**: Create a new chess game with optional wager
   - Parameters: game_id, wager_amount, game_type (PvP or PvAI)
   - Creates game account and move log account
   - For PvP: Sets status to WaitingForOpponent
   - For PvAI: Sets status to Active immediately
   - If wager_amount > 0: Transfers SOL to escrow PDA

3. **JoinGame**: Join an existing PvP game
   - Requires matching wager amount to be deposited
   - Updates game status to Active
   - Sets black player pubkey

4. **RecordMove**: Record a chess move on-chain
   - Validates it's the player's turn
   - For PvAI: Validates AI authority for black moves
   - Updates game FEN state and move count
   - Appends move to move log

5. **FinalizeGame**: End a game and distribute wagers
   - Determines winner or draw
   - Distributes escrowed SOL to winner(s)
   - Updates player ELO ratings
   - Updates win/loss statistics

6. **WithdrawExpiredWager**: Claim wagers from expired games
   - Allows players to reclaim their wager if opponent never joined
   - Prevents funds from being locked indefinitely

### Accounts

**Game Account**:
- game_id: Unique identifier for the game
- white: Public key of white player
- black: Public key of black player (or AI authority)
- status: WaitingForOpponent, Active, Finished, or Expired
- result: None, Winner(pubkey), or Draw
- fen: Current board state in FEN notation
- move_count: Number of moves played
- turn: Current turn number
- wager_amount: Amount of SOL wagered
- game_type: PvP or PvAI

**PlayerProfile Account**:
- authority: Player's public key
- elo: ELO rating (starts at 1200)
- wins: Total wins
- losses: Total losses
- games_played: Total games played

**MoveLog Account**:
- game_id: Reference to game
- moves: Vector of move strings in UCI format

### PDAs (Program Derived Addresses)

All PDAs are derived using the program ID and specific seeds:

- Game PDA: `["game", game_id.to_le_bytes()]`
- Move Log PDA: `["move_log", game_id.to_le_bytes()]`
- Escrow PDA: `["wager_escrow", game_id.to_le_bytes()]`
- Player Profile PDA: `["profile", player_pubkey]`

### Building the Program

```bash
cd programs/xfchess-game
anchor build
```

### Deploying to Devnet

```bash
solana config set --url https://api.devnet.solana.com
solana program deploy target/deploy/xfchess_game.so --url devnet
```

### Testing via CLI

Use the provided batch script to test the deployed program:

```bash
test_wager_cli.bat
```

This script will:
1. Check wallet balances
2. Generate a random game ID
3. Display the program status
4. Show account information

## Project Structure

```
XFChess/
├── programs/xfchess-game/     # Solana Anchor program
│   ├── src/
│   │   ├── lib.rs            # Program entry point
│   │   ├── instructions/     # Instruction handlers
│   │   ├── state/            # Account structures
│   │   ├── constants.rs      # Program constants
│   │   └── errors.rs         # Error codes
│   └── tests/
│       └── game_tests.rs     # Rust unit tests
├── src/                       # Main game source
│   ├── engine/               # Chess engine
│   ├── game/                 # Game systems and logic
│   ├── multiplayer/          # P2P networking
│   ├── solana/               # Solana client integration
│   ├── rendering/            # 3D rendering
│   └── ui/                   # User interface
├── crates/                    # Additional crates
│   ├── solana-chess-client/  # Solana RPC client
│   ├── braid-iroh/           # P2P networking
│   └── chess_engine/         # Core chess engine
├── web-solana/               # Web frontend
└── scripts/                  # Deployment and test scripts
```

## Getting Started

### Prerequisites

- Rust (latest stable)
- Solana CLI
- Anchor Framework
- Node.js (for web frontend)

### Running the Game

```bash
# Build the project
cargo build --release

# Run with local wallet
cargo run --release

# Deploy to devnet
./deploy_devnet.bat
```

### Running Tests

```bash
# Rust unit tests
cargo test

# Solana program tests
cd programs/xfchess-game
anchor test

# CLI integration test
./test_wager_cli.bat
```

## Features

### Single Player
- Play against Stockfish AI engine
- Multiple difficulty levels
- Move validation and legal move highlighting

### Multiplayer
- Peer-to-peer gameplay using Iroh networking
- Session key delegation for gasless moves
- Real-time synchronization

### Blockchain Integration
- Wager-based games with SOL escrow
- On-chain move recording
- ELO rating system
- Anti-cheat through state anchoring

## Technical Details

### Chess Engine
- Bitboard-based move generation
- Alpha-beta pruning with quiescence search
- Iterative deepening
- Transposition tables

### Networking
- Braid protocol for state synchronization
- Iroh for peer-to-peer connectivity
- Ephemeral rollups for low-latency gameplay

### Rendering
- Bevy game engine
- 3D piece models
- Custom shaders for board and pieces

## License

This project is licensed under the MIT License.

## Contributing

Contributions are welcome. Please ensure your code follows the existing patterns and includes appropriate tests.

## Resources

- [Solana Documentation](https://docs.solana.com/)
- [Anchor Framework](https://book.anchor-lang.com/)
- [Bevy Engine](https://bevyengine.org/)
- [Iroh P2P](https://iroh.computer/)
