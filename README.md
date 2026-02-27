# XFChess

**Decentralized Chess with Ephemeral Rollups on Solana**

Play chess competitively with real stakes. Every move is recorded on-chain for provable fairness, powered by MagicBlock ER for sub-second gameplay.

<p align="center">
  <img src="assets/screenshots/gameplay_1.png" width="45%" alt="XFChess Gameplay" />
  <img src="assets/screenshots/gameplay_2.png" width="45%" alt="XFChess Board View" />
</p>

## Quick Start

### Play a Wager Game

1. **Start both player UIs:**
   ```bash
   magicblock_e2e_test.bat
   ```

2. **Player 1** (http://localhost:5173):
   - Connect wallet → Create wager game → Copy Game ID

3. **Player 2** (http://localhost:5174):
   - Connect wallet → Join with Game ID

4. **Both players:**
   - Click "Launch Game" → Download session JSON
   - Run: `launch_game_with_session.bat xfchess_session_<game_id>.json`

5. **Play!** Moves sync via Solana, winner receives payout.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         XFChess                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐  │
│  │   Website    │      │  Web Lobby   │      │ Native Game  │  │
│  │  (React)     │<---->|  (React +    |<---->|  (Bevy +     │  │
│  │  Marketing   │      │   Anchor)    │      │   Solana)    │  │
│  └──────────────┘      └──────────────┘      └──────────────┘  │
│         │                     │                     │          │
│         └─────────────────────┼─────────────────────┘          │
│                               v                                │
│                    ┌──────────────────────┐                   │
│                    │   Solana Devnet      │                   │
│                    │   Program: xfchess   │                   │
│                    └──────────────────────┘                   │
│                               │                                │
│                    ┌──────────────────────┐                   │
│                    │   MagicBlock ER      │                   │
│                    │   (Optional)         │                   │
│                    └──────────────────────┘                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
XFChess/
├── programs/xfchess-game/     # Solana smart contract
├── src/                        # Native game (Rust/Bevy)
│   ├── game/                   # Chess mechanics
│   ├── multiplayer/            # P2P networking
│   ├── solana/                 # Blockchain client
│   └── rendering/              # 3D graphics
├── web-react/                  # Marketing website
├── web-solana/                 # Game lobby/wallet
└── crates/                     # Shared libraries
```

## Key Features

- **Wager Games** - Bet SOL on chess matches
- **On-Chain Moves** - Every move recorded on Solana
- **P2P Networking** - Fast move relay via Iroh
- **MagicBlock ER** - Sub-second delegated gameplay
- **Session Keys** - Secure ephemeral signing
- **3D Graphics** - Beautiful chess board with Bevy

## Program ID

```
AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp
```

Deployed on Solana Devnet.

## Building

### Prerequisites
- Rust 1.75+
- Node.js 18+
- Solana CLI (optional)

### Native Game
```bash
# Standard build (with Solana)
cargo build --release

# Without Solana (singleplayer only)
cargo build --release --no-default-features
```

### Web UIs
```bash
# Marketing site
cd web-react && npm install && npm run dev

# Game lobby
cd web-solana && npm install && npm run dev
```

## Documentation

Each folder contains detailed README:

- [`programs/xfchess-game/`](programs/xfchess-game/README.md) - Smart contract
- [`src/`](src/README.md) - Native game
- [`src/solana/`](src/solana/README.md) - Blockchain integration
- [`src/multiplayer/`](src/multiplayer/README.md) - P2P networking
- [`web-solana/`](web-solana/README.md) - Game lobby
- [`web-react/`](web-react/README.md) - Marketing site

## Testing

### Multiplayer Flow
```bash
# Start both UIs and test the full flow
magicblock_e2e_test.bat
```

### Solana Program
```bash
cd programs/xfchess-game
anchor test
```

### Native Game
```bash
# Singleplayer
cargo run

# With Solana session
cargo run -- --session-config session.json
```

## Technology Stack

- **Blockchain:** Solana, Anchor, MagicBlock ER
- **Game Engine:** Bevy (Rust)
- **P2P:** Iroh, Braid protocol
- **Frontend:** React, Vite
- **Contracts:** Rust (Anchor)

## License

MIT/Apache-2.0

## Links

- Website: https://xfchess.io (coming soon)
- Devnet: https://explorer.solana.com/address/AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp?cluster=devnet
- MagicBlock: https://docs.magicblock.gg/

---

**Play Anywhere. Own your History.**
