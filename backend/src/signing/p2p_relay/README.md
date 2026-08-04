# backend/src/signing/p2p_relay

Server-side state for Iroh P2P relay sessions: game announcement/discovery and node-ID
exchange so clients behind NAT can connect. The QUIC transport itself is
[crates/zarathustra_net/braid-iroh](../../../../crates/zarathustra_net/braid-iroh/).

## Files

| File | Contents |
|------|----------|
| [state.rs](state.rs) | In-memory relay session registry, keyed by game ID |
| [routes.rs](routes.rs) | Announce / discover / join endpoints |
| [types.rs](types.rs) | Wire types (node IDs, session descriptors) |

## Invariants

- Relay state is **in-memory only** — a backend restart drops live relay sessions
  (clients re-announce); nothing here is a source of truth.
- `node_id` identifies the transport; player identity is the wallet (or guest
  keypair) — don't conflate them.
