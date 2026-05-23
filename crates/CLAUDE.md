# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Crate inventory

| Crate | Key constraint | Used by |
|-------|---------------|---------|
| `nimzovich_engine` | `std` feature for game client; `no_std`-compatible for on-chain | Game client, backend, `chess-logic-on-chain` |
| `chess-logic-on-chain` | **must stay `no_std`** | Solana program (`move-validation` feature) |
| `shared` | Bevy + serde types | Game client, backend |
| `backend-types` | Serde-only, no Bevy | Backend, web frontend (via JSON) |
| `solana-chess-client` | Anchor + Solana SDK | Game client (`--features solana`) |
| `braid-core` | HTTP-209 Braid protocol primitives | All braid-* crates |
| `braid-http` | reqwest-based Braid client | Game client |
| `braid-iroh` | Iroh QUIC transport for Braid | Game client, backend |
| `braid_uri` | Typed URIs + move messages | Game client, backend |
| `iroh-gossip` | Gossip broadcast over Iroh | Backend relay |
| `iroh-h3`, `iroh-h3-axum`, `iroh-h3-client` | HTTP/3 layer over Iroh | Backend |
| `xfchess-braid-server` | Axum integration for HTTP-209 subscribe | Backend |
| `swiss-pairing` | FIDE Dutch Swiss algorithm | Backend (`network` feature adds Axum routes) |

## Critical constraints

**`chess-logic-on-chain` must remain `no_std`**: This crate is compiled into the Solana program. Any accidental `std` import will break the program build. Use `bytemuck` for data layout and avoid anything that touches the allocator (unless you enable the `alloc` feature carefully).

**`nimzovich_engine` has two personalities**: With `features = ["std", "search"]` it runs full alpha-beta search. Without `std` it provides only move generation (used on-chain). Keep the feature boundary clean.

**Braid protocol**: HTTP-209 is a streaming subscribe protocol. `braid-core` holds the codec; `braid-http` wraps reqwest for clients; `braid-iroh` tunnels it over QUIC. Do not conflate HTTP-209 subscriptions with WebSocket connections — they serve different roles (Braid for board state sync, WebSocket for auth/signaling).

**`swiss-pairing` networking**: The `network` feature adds Axum handlers to the crate. Only enable it in the backend, not in the game client.

## Adding a new shared crate

1. Create `crates/<name>/Cargo.toml` and `src/lib.rs`.
2. Add to `workspace.dependencies` in the root `Cargo.toml`.
3. Reference via `{ path = "crates/<name>" }` in consuming crate's `Cargo.toml`.
4. Exclude from the workspace in root `Cargo.toml` if it should not be compiled by default (like `solana-contract-fuzzer`).
