# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

XFChess is a 3D chess game built with Rust + Bevy and Solana blockchain integration. It consists of five major components that share a Cargo workspace:

| Component | Path | Tech |
|-----------|------|------|
| Game client | `src/` | Bevy 0.18, ECS, Iroh P2P |
| Backend API | `backend/` | Axum 0.8, SQLite/SQLx, Tokio |
| Solana program | `programs/xfchess-game/` | Anchor 0.31, Ephemeral Rollups |
| Web frontend | `web-solana/` | React 19, Vite, Chakra UI |
| Desktop wrapper | `tauri/` | Tauri 2.1 |

Shared library crates live in `crates/` — see [crates/CLAUDE.md](crates/CLAUDE.md).

## Commands

### Game client

```bash
# Dev build (fast compile, deps at opt-level=3)
cargo build

# Run game (no blockchain)
cargo run

# Run game with Solana features
cargo run --features solana

# Release build
cargo build --release

# Run specific binary
cargo run --bin pda --features solana
cargo run --bin debugger
```

### Backend

```bash
cd backend
cargo build
cargo run --bin backend          # Main API server
cargo run --bin signing-server   # Standalone signing service
cargo run --bin tournament_admin # Tournament management CLI
```

### Tests

```bash
# All workspace tests
cargo test

# Single test by name
cargo test test_name

# Solana program tests
cargo test -p xfchess-game

# A specific program test file
cargo test -p xfchess-game --test smoke_tests
cargo test -p xfchess-game --test security_tests

# Backend tests
cargo test -p backend
```

### Lint & format

```bash
cargo fmt
cargo clippy
```

### Web frontend

```bash
cd web-solana
npm install
npm run dev      # Dev server
npm run build    # Production build (runs tsc then vite)
npm run lint     # ESLint
npm run preview  # Preview production build
```

### Solana program

```bash
# Build (size-optimized via anchor)
scripts\build_program.bat
# or directly:
anchor build

# Deploy to devnet
anchor deploy

# Deploy to mainnet (~6.5 SOL)
solana program deploy target/deploy/xfchess_game.so
```

### Full stack

```bash
# Build everything (game + backend + web + Solana program)
scripts\build.bat

# Run local dev stack with monitoring (Prometheus, Grafana)
scripts\run_offline.bat

# Production packaging for Hetzner VPS
scripts\package_backend_hetzner.bat

# Docker monitoring stack
docker-compose up -d
```

## Architecture

### Data flow

```
Player -> Bevy client -> Backend API (Axum) -> Solana RPC
                      -> P2P relay (Iroh/QUIC) -> Opponent client
                                              -> Ephemeral Rollups (MagicBlock, sub-second moves)
```

Moves are validated on-chain via `chess-logic-on-chain` (no_std). The Solana program stores game state as FEN + move history. Session delegation allows passwordless play — a session key co-signs moves on behalf of the wallet owner.

### Game client (`src/`)

Bevy ECS app. Key modules:

- `core/` — app lifecycle, crash reporting, `AppState` enum (`Splash → MainMenu → Game → Pause`)
- `game/` — board state, FEN management, move validation, check/checkmate detection
- `engine/` — delegates to `nimzovich_engine` crate for AI moves
- `multiplayer/` — WebSocket auth + Iroh P2P relay
- `rendering/` — isometric 3D board via Bevy; `graphics_quality.rs` controls settings
- `solana/` — gated behind `--features solana`; wraps `solana-chess-client` crate

`build.rs` copies assets and injects the backend URL at compile time.

### Backend (`backend/`)

Axum server with four binaries. Key areas:

- `src/signing/` — Solana transaction building, blinks API, anti-cheat (IP-based), compliance checks (UK/Brazil/Germany/Canada)
- `src/signing/routes/` — HTTP handlers for matchmaking, ratings, tournaments, PDF mailer
- `src/db/` — SQLite via SQLx with migrations in `backend/migrations/`
- `src/tasks/` — background tasks (tournament auto-advancement, auto game settlement, auto prize distribution)
- `src/telemetry/` — Prometheus metrics exposed at `/metrics`
- `src/signing/storage/tournament.rs` — SQLite-backed tournament store (survives restarts)

The backend holds in-memory P2P relay state via `braid-iroh`. The `signing/` module builds unsigned Solana transactions that the client signs locally — the backend never holds private keys.

### Solana program (`programs/xfchess-game/`)

Anchor 0.31 program. Program ID: `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU` (localnet + devnet).

Instruction groups (each in its own subdirectory under `src/`):

- `account_ix/` — profile init, fee vault, session keys, ELO updates
- `game_ix/` — create/join/cancel/resign/timeout/finalize
- `moves_ix/` — `record_move` (called on ER, sub-second)
- `delegation_ix/` — delegate/undelegate game accounts to Ephemeral Rollups, session key auth
- `tournament_ix/` — full tournament lifecycle: initialize → register → start → matches → prizes
- `governance_ix/` — dispute/resolve/claim stale dispute
- `crank_ix/` — MagicBlock scheduled time checks (feature-gated `cranks`)

The Solana program uses `opt-level = "z"` for size. The `move-validation` feature pulls in `chess-logic-on-chain` (no_std) to validate moves on-chain.

### Crates (`crates/`)

See [crates/CLAUDE.md](crates/CLAUDE.md) for per-crate details. Key ones:

- `nimzovich_engine` — chess AI (search + move gen), used by game client and backend
- `chess-logic-on-chain` — no_std move validation, used inside the Solana program
- `braid-*` + `braid_uri` — HTTP-209 Braid protocol for live game subscriptions (P2P relay)
- `swiss-pairing` — FIDE Dutch Swiss tournament pairing
- `solana-chess-client` — client-side transaction builders for the Solana program
- `shared` — types shared across game client and backend

## Key conventions

- **Feature flags**: Solana-dependent code in the game client is gated behind `--features solana`. Never import Solana types unconditionally at the top level of `src/`.
- **No_std constraint**: `chess-logic-on-chain` and `nimzovich_engine` (when used on-chain) must remain `no_std`. Do not add `std`-only dependencies to these crates.
- **Ephemeral Rollup lifecycle**: Game accounts must be delegated before ER moves are recorded, then undelegated back to mainnet to finalize. `delegation_ix/` handles both sides; `process_undelegation` is called by the ER infrastructure automatically.
- **Commit style**: `type: description` — types are `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`.
- **AI-assisted PRs**: Include the prompts used and AI tool name in the PR description or commit message. Manual testing proof (screenshots/video) is required.
- **License**: AGPL-3.0. Modified versions served over a network must provide source.

## Observability

- Prometheus metrics: `GET /metrics`
- Health check: `GET /health`
- Grafana: `http://localhost:3000` (when running `docker-compose up -d`)
- Transaction debug: `GET /api/debug/transaction/:signature`

See [docs/OBSERVABILITY.md](docs/OBSERVABILITY.md) for dashboard setup.
