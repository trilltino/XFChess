# Web-to-Native Game Bridge Architecture

## Overview
Allow users to connect wallet in web app, then launch native Bevy game with Solana integration.

## Components

### 1. Web App Extension (web-solana/)
- Wallet connection (existing)
- "Launch Native Game" button
- Generate game session JWT
- API endpoint to validate sessions

### 2. Launcher Batch File (solana_game_launcher.bat)
- Receives session token from web
- Launches native game with token
- Passes wallet pubkey to game

### 3. Native Game Integration (src/)
- Accept session token via CLI arg
- Validate session with web API
- Use delegated wallet for transactions
- Integrate with MagicBlock ephemeral rollups

## Flow

1. User connects wallet on web
2. Web generates session: `{wallet_pubkey, session_id, expiry, signature}`
3. User clicks "Launch Game"
4. Browser calls custom protocol: `xfchess://launch?token=xyz`
5. Batch file receives token, launches: `xfchess.exe --session xyz`
6. Game validates token with web backend
7. Game uses wallet pubkey for Solana transactions

## Files to Create

1. `web-solana/src/components/GameLauncher.tsx` - Launch button + session gen
2. `web-solana/src/pages/api/session.ts` - Session validation API
3. `solana_game_launcher.bat` - Windows launcher
4. `src/bin/launcher.rs` - Native game launcher entry point
5. `src/solana/session.rs` - Session validation in game

## Security
- Sessions expire after 1 hour
- Signed by web backend
- One-time use tokens
- Game validates signature before using wallet
