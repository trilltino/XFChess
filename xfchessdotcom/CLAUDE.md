# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

React 19 + TypeScript web frontend for XFChess. Provides wallet-based tournament registration, game browsing, and Solana Blinks integration. Talks to the backend API and directly to Solana RPC via `@coral-xyz/anchor`.

## Commands

```bash
npm install
npm run dev       # Vite dev server
npm run build     # tsc -b && vite build → dist/
npm run lint      # ESLint
npm run preview   # Serve production build locally
```

## Key dependencies

| Package | Role |
|---------|------|
| `@coral-xyz/anchor` | Solana program client (types from `xfchess-game` IDL) |
| `@magicblock-labs/ephemeral-rollups-sdk` | ER transaction helpers |
| `@solana/wallet-adapter-*` | Wallet connection (Phantom, WalletConnect, etc.) |
| `react-router-dom` v7 | Client-side routing |
| `framer-motion` | Animations |

## Architecture notes

- The Anchor IDL for `xfchess-game` must be regenerated (`anchor build`) and copied here whenever the Solana program instructions change.
- Wallet context wraps the entire app; access the connected wallet via `useWallet()` from `@solana/wallet-adapter-react`.
- Transactions are built using `@coral-xyz/anchor`'s `Program` class — never construct raw transactions manually. The backend signing routes return partially-built transactions that the frontend completes and signs.
- Ephemeral Rollup moves use the `@magicblock-labs/ephemeral-rollups-sdk` to target the MagicBlock RPC endpoint instead of mainnet.
