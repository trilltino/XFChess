# XFChess Web Lobby

## Purpose
The Web Lobby is the bridge between the player's web browser and the native XFChess game client. It handles wallet connection, game creation/joining, and generates session keys for the native game to use.

## Impact on Game
This is the **entry point** for wager-based multiplayer games:
- **Wallet Integration:** Connects Solana wallets (Phantom, Solflare, etc.)
- **Game Lifecycle:** Creates and joins wager games on-chain
- **Session Management:** Generates ephemeral keys for secure game signing
- **Game Launch:** Triggers native client with session configuration

## Architecture

```
Player Browser    Web-Solana    Solana Devnet    Native Game
      |               |               |               |
      |-- Connect ---->|               |               |
      |   Wallet      |               |               |
      |<--------------+               |               |
      |               |               |               |
      |-- Create Game->|               |               |
      |               |-- Transaction->|               |
      |               |<-- Game PDA --+               |
      |<-- Game ID ---+               |               |
      |               |               |               |
      |-- Launch Game->|               |               |
      |   (generates   |               |               |
      |    session)    |               |               |
      |<-- Download ---+               |               |
      |   session.json |               |               |
      |               |               |               |
      |---------------------------------------------->|
      |               |               |   Launch with
      |               |               |   --session-config
```

## Key Components

### Pages
- **Lobby** (`src/pages/Lobby.tsx`) - Game listing and creation
- **GameDetail** (`src/pages/GameDetail.tsx`) - Game status and launch
- **Home** (`src/pages/Home.tsx`) - Landing with GameLauncher

### Hooks
- **useGameProgram** (`src/hooks/useGameProgram.ts`) - Solana program interaction via Anchor
- **useGameLauncher** (`src/hooks/useGameLauncher.ts`) - Session generation and game launch
- **useMagicBlock** (`src/hooks/useMagicBlock.ts`) - ER delegation management

### Components
- **GameLauncher** (`src/components/GameLauncher.tsx`) - Wallet session creation

## Game Flow

### 1. Player 1 Creates Game
```typescript
const { createGame } = useGameProgram();
const result = await createGame(0.01, 'pvp');
// Returns: { signature, gameId, gamePDA }
```

### 2. Player 2 Joins Game
```typescript
const { joinGame } = useGameProgram();
const result = await joinGame(gameIdBN);
// Both players now have stake in escrow
```

### 3. Launch Game Client
```typescript
const { launchGame } = useGameLauncher();
const result = await launchGame(gameId, playerColor, wager);
// Downloads: xfchess_session_<game_id>.json
```

## Session JSON Format
```json
{
  "gameId": "1234567890",
  "playerColor": "white",
  "sessionKey": "base58_encoded_secret",
  "sessionPubkey": "base58_encoded_pubkey",
  "nodeId": "node_abc123",
  "rpcUrl": "https://api.devnet.solana.com",
  "gamePda": "GamePDA_address",
  "wagerAmount": 0.01,
  "opponentPubkey": "Optional_opponent_address"
}
```

## Usage

### Development
```bash
cd web-solana
npm install
npm run dev
```

### Build for Production
```bash
npm run build
```

## Configuration

### Environment Variables
Create `.env`:
```bash
VITE_SOLANA_RPC_URL=https://api.devnet.solana.com
VITE_PROGRAM_ID=AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp
```

### Wallet Adapter
Configured for:
- Phantom
- Solflare
- Backpack
- Sollet

## Dependencies
- React 18
- @solana/wallet-adapter-react
- @coral-xyz/anchor
- @magicblock-labs/gum-sdk (for ER sessions)

## Testing Multiplayer

1. Start both UIs:
```bash
../magicblock_e2e_test.bat
```

2. Player 1: http://localhost:5173
   - Connect wallet → Create game → Copy Game ID

3. Player 2: http://localhost:5174
   - Connect wallet → Join game → Paste Game ID

4. Both: Click "Launch Game" → Run batch file

## Integration with Native Game

The web UI downloads a session JSON that the native game reads via:
```bash
xfchess.exe --session-config xfchess_session_<game_id>.json
```

This allows the native game to:
- Connect to the correct Solana RPC
- Use ephemeral session keys (not exposing main wallet)
- Know which game PDA to interact with
- Identify as White or Black
