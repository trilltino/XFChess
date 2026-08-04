# braid-iroh

**Braid-HTTP over Iroh** — runs the Braid synchronization protocol over a peer-to-peer QUIC
transport instead of (or alongside) plain HTTP. This is the low-latency, NAT-traversing
transport for XFChess P2P play and for gossip-based matchmaking.

Built on [`braid-core`](../braid-core) (protocol types) and the vendored
[`iroh-gossip`](../iroh-gossip) crate (topic broadcast). Part of the XFChess Rust Braid
implementation (a port of the braid.org JavaScript reference).

## Why QUIC instead of HTTP

`braid-http` speaks Braid to a server over TCP/HTTP. `braid-iroh` speaks the *same* protocol
between peers over **Iroh** (QUIC + hole-punching + relay fallback). Benefits for real-time
play:

- **Direct peer connections** when NAT traversal succeeds — no server round-trip per move.
- **Gossip topics** for one-to-many fan-out (matchmaking announcements, spectator feeds).
- **Discovery** via Iroh's discovery service, so peers find each other by node id.

It is an *alternative transport for the same Braid resources*, not a different protocol.
The Solana/Ephemeral-Rollup move path does not use this layer.

## Public API

```rust,no_run
use braid_iroh::{spawn_node, DiscoveryConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Spawn a node.
    let state = spawn_node(
        "alice",                 // name (seeds a deterministic key for alice/bob; hashed otherwise)
        Some(8080),              // optional proxy port
        None,                    // optional explicit secret key
        DiscoveryConfig::Default,
    ).await?;

    println!("node id: {}", state.node_id);

    // Subscribe to a game topic (optionally bootstrapping from known peers).
    let mut moves = state.peer.subscribe("/game/ABCD42/moves", vec![]).await?;
    let _ = &mut moves;
    Ok(())
}
```

| Item | Purpose |
|------|---------|
| `BraidIrohNode` | An Iroh endpoint speaking the Braid protocol; `spawn`, `subscribe(topic, bootstrap_peers)`, `node_id()` |
| `BraidIrohState` | Shareable handle: `{ peer, node_id, node_name }` |
| `BraidIrohConfig` | `{ discovery, secret_key, proxy_config, data_dir }` |
| `spawn_node(name, port, key, discovery)` | Convenience constructor → `Arc<BraidIrohState>` (subscribe to topics via `state.peer`) |
| `get_or_create_secret_key(name)` | Deterministic keys for `alice`/`bob` in tests, blake3-hashed otherwise |
| `DiscoveryConfig` | How peers are discovered |

## Module map

| Module | Contents |
|--------|----------|
| `node.rs` | `BraidIrohNode`, spawn/config, topic subscription |
| `subscription.rs` | Subscription lifecycle over gossip |
| `discovery.rs` | Peer discovery configuration |
| `tournament.rs` | Tournament-topic helpers |
| `protocol.rs`, `proxy.rs` | *(feature `proxy`)* a local HTTP→Iroh proxy so HTTP-only callers can reach Braid-over-Iroh resources |

## Features

| Feature | Effect |
|---------|--------|
| *(default)* | Node + gossip subscription |
| `proxy` | Enables `protocol`/`proxy`: a local HTTP proxy bridging plain HTTP clients to Iroh peers |

## Role in XFChess

```
src/multiplayer/        (game client)
        │
        ▼
   braid-iroh           ◄── you are here (QUIC transport for Braid)
        │
        ├──► braid-core  (protocol types)
        └──► iroh-gossip (topic broadcast; vendored fork)
```

Used by `src/multiplayer/` for P2P Braid mode and gossip matchmaking. The backend relay also
bridges gossip topics into its Braid resource hub.

## Provenance

Rust work by the XFChess author over Iroh and the braid.org-derived protocol core. Protocol ©
the Braid working group; `iroh-gossip` is a vendored fork of the upstream crate (license
preserved in that crate). See `xfchess-braid-server/ATTRIBUTION.md`.
