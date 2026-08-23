# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Crate inventory

| Crate | Key constraint | Used by |
|-------|---------------|---------|
| `nimzovich_engine` | `std` feature for game client; `no_std`-compatible for on-chain | Game client, backend, `chess-logic-on-chain` |
| `nimzovich-uci` | UCI protocol adapter binary for the engine | Match testing via `cutechess-cli`, not linked into the app |
| `chess-logic-on-chain` | **must stay `no_std`** | Solana program (`move-validation` feature) |
| `xfchess-anticheat` | Engine-move-correlation anti-cheat | Backend |
| `solana-chess-client` | Anchor + Solana SDK | Game client (`--features solana`) |
| `er-cu-benchmark` | Compute-unit/RPC load-test binaries against MagicBlock ER | Standalone benchmarking tool, not linked into the app |
| `braid-core` | Thin HTTP-209 Braid facade over `braid-http` | All braid-* crates |
| `braid-http` | reqwest-based Braid-HTTP 209 client (Rust port of braid.org JS) | Game client |
| `braid-iroh` | Iroh QUIC transport for Braid | Game client, backend |
| `braid_chess` | Typed chess messages + resources + publish/subscribe (was `braid_uri`); also the **tournament vocabulary** (`SwissMessage`, `TournamentResource`) — application types, moved here out of the `braid-iroh` transport crate | Game client, backend |
| `iroh-h3`, `iroh-h3-axum`, `iroh-h3-client` | HTTP/3 layer over Iroh | Live: `braid-iroh`'s `proxy` feature mounts `iroh-h3-axum`'s handler on every spawned node (`braid-iroh/src/node.rs`) and `iroh-h3-client` backs the spectator TCP bridge (`src/multiplayer/systems.rs` sets `proxy_config: Some(..)`). The "not used by anything" note each crate's own README carries is stale for this repo — do not remove based on that alone. |
| `xfchess-braid-server` | Axum integration for HTTP-209 subscribe | Backend |
| `swiss-pairing` | FIDE Dutch Swiss algorithm (simplified: no transposition/exchange search) | Backend |

## Critical constraints

**`chess-logic-on-chain` must remain `no_std`**: This crate is compiled into the Solana program. Any accidental `std` import will break the program build. Use `bytemuck` for data layout and avoid anything that touches the allocator (unless you enable the `alloc` feature carefully).

**`nimzovich_engine` has two personalities**: With `features = ["std", "search"]` it runs full alpha-beta search. Without `std` it provides only move generation (used on-chain). Keep the feature boundary clean.

**Braid protocol**: HTTP-209 is a streaming subscribe protocol (a Rust port of the braid.org JS reference). `braid-http` holds the protocol codec + reqwest/WASM client; `braid-core` is a thin re-export facade over it; `braid-iroh` tunnels the same protocol over QUIC; `xfchess-braid-server` is the Axum-native server/fan-out hub. Note the upstream CRDT/OT merge engine was removed — XFChess is server-authoritative (JSON Patch for tournament docs, append-log for moves). Do not conflate HTTP-209 subscriptions with WebSocket connections — they serve different roles (Braid for board/tournament state sync, WebSocket for auth/signaling).

**One write, two transports**: every tournament fact is written once, into a `ResourceHub` resource. The hub fans that update out to HTTP-209 subscribers (`/braid/*`, mounted in `infrastructure/router.rs`) *and*, through `ResourceHub::set_gossip_sink` (wired in `server.rs`), to P2P peers as the same versioned update. Do not add a second publication path beside a hub write — that is exactly what was removed here: `SwissService` emitted tagged-JSON `SwissMessage`s over gossip alongside its hub writes, with a SQLite `tournament_gossip_log` table for late-joiner replay. Both were dead: the broadcasts were guarded on a `gossip` field that was never set, and nothing ever called `persist_message`. Subscribe-replay covers the case they were aiming at.

**The tournament path has no consumer yet.** `/braid/tournament/{id}/*` is served and tested, and the hub's updates reach gossip, but nothing subscribes: the web frontend has no Braid client, and the game client's `spawn_swiss_subscription` (`src/multiplayer/solana/tournament.rs`) is defined and never called. Wiring a consumer is what makes this visible to a user; until then it is a working server half.

**Transport crates hold no application vocabulary**: `braid-iroh` carries Braid over QUIC and nothing else. Game- and tournament-specific message types belong in `braid_chess`.

## Adding a new shared crate

1. Create `crates/<name>/Cargo.toml` and `src/lib.rs`.
2. Add to `workspace.dependencies` in the root `Cargo.toml`.
3. Reference via `{ path = "crates/<name>" }` in consuming crate's `Cargo.toml`.
4. Exclude from the workspace in root `Cargo.toml` only if it must not be compiled by default. Note: an excluded crate **cannot** use `dep.workspace = true` inheritance (it has no workspace root), so excluding trades drift-protection for build-time. Prefer keeping crates as members unless the build-time cost is real and unique to that crate.
