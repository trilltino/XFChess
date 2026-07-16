# backend/ — signing & tournament server

Axum 0.8 HTTP server that sits between the game client / web frontend and Solana.
It builds (but never signs) Solana transactions, manages tournament state, relays
P2P connections, and exposes Prometheus metrics. It never holds player private keys.

## Role in XFChess

```
Bevy client / web-solana ──HTTP──> backend ──RPC──> Solana (mainnet + MagicBlock ER)
                          ──WS───> auth / signaling
                          ──QUIC──> p2p_relay (braid-iroh) ──> opponent client
```

The backend returns serialized **unsigned** transactions; the client signs with its
wallet (or delegated session key) and either sends them back for relay or broadcasts
directly. Background tasks settle finished games and distribute tournament prizes
on-chain without any client action.

## Binaries

| Binary | Entry point | Purpose |
|--------|-------------|---------|
| `signing-server` (alias `signing-server-http`) | [src/signing_server.rs](src/signing_server.rs) | **The API server** — everything described here |
| `backend` | [src/main.rs](src/main.rs) | Stub; prints "use signing-server instead" |
| `vps_admin` | [src/bin/vps_admin.rs](src/bin/vps_admin.rs) | VPS admin tasks |
| `tournament_admin` | [src/bin/tournament_admin.rs](src/bin/tournament_admin.rs) | Tournament management CLI |
| `import_puzzles` | [src/bin/import_puzzles.rs](src/bin/import_puzzles.rs) | Puzzle DB importer |

```bash
cd backend
cargo run --bin signing-server   # the API server, :8090
cargo test -p backend
```

## Module map

| Module | Purpose |
|--------|---------|
| [src/signing/](src/signing/README.md) | Core domain: transaction building, auth, routes, compliance, relay |
| [src/signing/routes/](src/signing/routes/) | HTTP handlers: matchmaking, tournaments, ratings, disputes, KYC, mailer |
| [src/signing/solana/](src/signing/solana/) | Transaction building, RPC routing (mainnet vs ER), tx debug API |
| [src/signing/swiss/](src/signing/swiss/) | Swiss tournament orchestration (scoring: [SCORING.md](src/signing/swiss/SCORING.md)) |
| [src/db/](src/db/README.md) | SQLite via SQLx; migrations in [migrations/](migrations/) |
| [src/tasks/](src/tasks/) | Background workers: settlement, prize distribution, matchmaking, anticheat, archiver |
| [src/telemetry/](src/telemetry/) | Prometheus metrics, logging, HTTP middleware |
| [src/infrastructure/](src/infrastructure/README.md) | Router assembly, DB pool, auth middleware, task spawning |

## Example — session-key game flow

The passwordless play loop the game client drives (routes in
[src/signing/routes/main.rs](src/signing/routes/main.rs)):

```
POST /session/create     -> unsigned create_game tx + session keypair delegation
POST /session/activate   -> activates the delegated session key
POST /move/record        -> backend co-signs move for MagicBlock ER, sub-second ack
POST /game/undelegate    -> commit ER state back to mainnet
POST /game/finalize      -> settle wager (also done automatically by tasks/settlement_worker.rs)
```

## Invariants

- **Never add private-key handling.** The signing model is build-unsigned/return-serialized.
- Tournament state of record is [src/signing/storage/tournament.rs](src/signing/storage/tournament.rs)
  (SQLite `tournaments` table, JSON blob per record) — survives restarts.
- Wager routes check [src/signing/cacf/](src/signing/cacf/) jurisdiction rules (UK, Brazil,
  Germany, Canada) before building transactions.
- Schema changes = new numbered file in [migrations/](migrations/); never edit old ones.

Observability: `GET /health`, `GET /metrics` (Prometheus). More detail in
[CLAUDE.md](CLAUDE.md) and [src/README.md](src/README.md).
