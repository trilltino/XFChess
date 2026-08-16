# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

The XFChess backend is an Axum 0.8 HTTP server that sits between the game client and Solana. It builds (but never signs) Solana transactions, manages tournament state, relays P2P connections, and exposes a Prometheus metrics endpoint. It never holds private keys.

## Binaries

| Binary | Entry point | Purpose |
|--------|-------------|---------|
| `signing-server` | `src/signing_server.rs` → `backend::server::run` | The API server (local-dev name) |
| `signing-server-http` | `src/signing_server_http.rs` → `backend::server::run` | Same server — ops/prod name (systemd, CI) |
| `backend` | `src/main.rs` | Stub — prints "use signing-server instead" |
| `vps_admin` | `src/bin/vps_admin.rs` | VPS admin tasks |
| `tournament_admin` | `src/bin/tournament_admin.rs` | CLI tournament management |
| `import_puzzles` | `src/bin/import_puzzles.rs` | Puzzle DB importer |

```bash
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
│   ├── social/          # Friends, presence, lobby invites
│   ├── swiss/           # Swiss-pairing tournament orchestrator (wraps swiss-pairing crate)
│   ├── blinks/           # Solana Blinks / actions API
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

**Game event log**: `signing/routes/game_log.rs` is a durable, ordered move/resign/draw/chat/session-info log served over Braid-HTTP 209 (`GET`/`PUT /game/:id/moves` and `/game/:id/chat`), backed by the `game_event_log` SQLite table plus an in-memory per-game `broadcast::Sender<BraidUpdate>` map (**not** `xfchess_braid_server::ResourceHub`/`AppendLog` — its JSON-Patch-wrapped bodies don't decode through `ChessSubscriber`, see the module doc comment for the regression test that proved it). It replaced `relay_bridge.rs`/`chat.rs` for move/resign/chat/session-info sync — those now push to subscribers instead of being polled, and survive a backend restart instead of being dropped. Direct P2P sync between clients (NAT traversal, QUIC) still goes over Iroh gossip via `braid-iroh`; this log is the durable backend-side fallback/catch-up path, not a replacement for gossip. `signing/p2p_relay/` is a **separate, still-live** poll-based mailbox for the lobby JOIN_ACK handshake only (pre-game, before a `game_id` exists) — it was never replaced and must not be deleted. Wagered-game participant claims (`SessionInfo`) are cross-checked against on-chain `Game.white`/`black` (`signing/solana/game_participants.rs`) before being trusted into the roster, closing a first-claim-wins race.

**CACF compliance**: Before building a wager transaction, the signing routes check the player's country code against `cacf/` rules. Adding new restricted jurisdictions means adding a file in that directory.

**Wallet identity, backend-verified**: `signing/routes/auth.rs`'s username/email/JWT login is an identity/UX layer on top of wallet identity — the wallet *is* the identity, and the game client fetches a JWT automatically on every wallet connection (`src/multiplayer/network/vps/client.rs`) before any session/lobby action. `require_relay_or_jwt` (`infrastructure/auth_middleware.rs`), which guards the session-key/move/finalize routes, **fails closed**: a request with neither a valid JWT nor a matching `RELAY_SHARED_SECRET` is rejected (401), not passed through. `record_move` additionally checks that the JWT's wallet matches the request's `mover_wallet`, so a valid credential for one wallet can't submit a move on behalf of another. Covered by `dual_accept_auth_guards_signing_endpoints` in `tests/e2e_api.rs`.

**Feature flags**:
- `ws_subscriber` (default) — enables WebSocket-based live game subscription
- `polling` — alternative polling mode for environments without WebSocket support

## Database

SQLite with SQLx 0.9. Migrations live in `backend/migrations/`. Run them with:

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
