# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The XFChess backend is an Axum 0.8 HTTP server that sits between the game client and Solana. It builds (but never signs) Solana transactions, manages tournament state, relays P2P connections, and exposes a Prometheus metrics endpoint. It never holds private keys.

## Binaries

| Binary | Entry point | Purpose |
|--------|-------------|---------|
| `backend` | `src/main.rs` | Main API server |
| `signing-server` | `src/signing_server.rs` | Standalone signing service |
| `vps_admin` | `src/bin/vps_admin.rs` | VPS admin tasks |
| `tournament_admin` | `src/bin/tournament_admin.rs` | CLI tournament management |

```bash
cargo run --bin backend
cargo run --bin signing-server
cargo test
cargo test test_name   # single test
```

## Module map

```
src/
├── signing/
│   ├── routes/          # HTTP handlers (matchmaking, ratings, tournament, PDF mailer)
│   ├── solana/          # Transaction building + RPC interaction
│   │   ├── telemetry.rs # Prometheus metric collection
│   │   └── debug.rs     # Transaction inspection API
│   ├── cacf/            # Compliance checks (UK, Brazil, Germany, Canada)
│   ├── p2p_relay/       # Iroh-based relay for multiplayer
│   ├── blinks*.rs       # Solana Blinks / actions API
│   └── storage/tournament.rs  # SQLite-backed tournament store
├── db/                  # SQLite via SQLx
├── tasks/               # Background tasks (tournament auto-advancement, auto settlement, prize distribution)
├── telemetry/           # Prometheus setup
└── error.rs             # Unified error type
```

## Critical design points

**Transaction signing model**: The backend builds unsigned Solana transactions and returns them serialised to the client. The client signs with their wallet and broadcasts. Never add private key handling here.

**Tournament state**: `signing/storage/tournament.rs` is the source of truth for live tournament data, persisted to SQLite (`tournaments` table, JSON blob per record) — it survives server restarts. The same DB also holds user sessions, profiles, and historical data.

**Automatic settlement**: `tasks/settlement_worker.rs` scans active game sessions every 30s, reads the Game PDA on-chain, and auto-submits `finalize_game` (wager payout) once a result is committed — clients never have to call `/game/finalize`. `tasks/tournament_scheduler.rs::spawn_prize_distributor` cranks the permissionless `distribute_tournament_prizes` instruction when a tournament completes, pushing SOL prizes to winners without claim transactions.

**P2P relay**: The `p2p_relay/` module manages Iroh node state. Each relay session is tied to a game ID. The braid-iroh crate handles the underlying QUIC transport.

**CACF compliance**: Before building a wager transaction, the signing routes check the player's country code against `cacf/` rules. Adding new restricted jurisdictions means adding a file in that directory.

**Feature flags**:
- `ws_subscriber` (default) — enables WebSocket-based live game subscription
- `polling` — alternative polling mode for environments without WebSocket support

## Database

SQLite with SQLx 0.8. Migrations live in `backend/migrations/`. Run them with:

```bash
sqlx migrate run
```

Schema changes require a new numbered migration file — never edit existing migrations.

## Testing

Uses `wiremock` for HTTP mocking and `tower` for in-process Axum testing. Integration tests that touch Solana should be run against devnet, not localnet.

```bash
cargo test -p backend
cargo test -p backend -- --test-thread=1   # if tests share SQLite state
```
