# braid-core

The **thin Braid-HTTP protocol surface** the XFChess workspace consumes. `braid-core` is a
small facade over [`braid-http`](../braid-http): it re-exports the protocol types and client
behind one stable import path and adds the local error type, so the rest of the
`zarathustra_net` stack depends on `braid-core` rather than reaching into `braid-http`
directly.

Part of the XFChess Rust implementation of the Braid protocol — a Rust port of the
[braid.org](https://braid.org) JavaScript reference.

## Scope

The crate is deliberately minimal: protocol vocabulary plus a thin trait layer, nothing
else. XFChess is **server-authoritative** — chess moves are an ordered append-log and
tournament documents are updated via JSON Patch (RFC 6902), with the relay server (and,
for staked games, the Solana program) as the single source of truth. That architecture
needs no client-side CRDT or operational-transform merge, no filesystem sync, and no
blob store, so `braid-core` carries none. Server-side HTTP-209 handling lives in
[`xfchess-braid-server`](../xfchess-braid-server), an Axum-native implementation.

## Public surface

```rust
// Always available:
pub use braid_core::{BraidError, Result};
pub use braid_core::{BraidRequest, BraidResponse, Patch, Update, Version};
pub use braid_core::types;            // braid_http::types

// With the `client` feature:
pub use braid_core::{BraidClient, ClientConfig, Subscription};
pub use braid_core::client;           // braid_http::client
```

### `core/` module

| File | Contents |
|------|----------|
| `traits.rs` | The runtime-abstraction traits: `BraidRuntime` (spawn/sleep/now), `BraidNetwork` (fetch/subscribe), `BraidStorage` (put/get/delete/list). These decouple protocol logic from the execution environment so the same code runs on Tokio (server) or WASM (browser). |
| `error.rs` | `BraidError` / `Result` for the core surface. |
| `mod.rs` | Re-exports the commonly used protocol types. |

## Where it sits

```
braid_chess, braid-iroh, … ──► braid-core ──► braid-http
                                  (facade)      (protocol impl)
```

## Provenance

Rust port by the XFChess author; protocol © the Braid working group (MIT OR Apache-2.0).
See `xfchess-braid-server/ATTRIBUTION.md`.
