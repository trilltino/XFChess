---
name: iroh
description: Implement or debug XFChess Iroh networking, authenticated QUIC connections, gossip, relay fallback and HTTP/3 adapters. Use for peer transport and connection lifecycle changes.
metadata:
  provenance: docs-derived
---

# Iroh, QUIC and HTTP/3

Start with `crates/zarathustra_net/`, its `UPSTREAM.md`, and
`src/multiplayer/network/`. The workspace pins Iroh 1.3.0 and separately pins
iroh-gossip; inspect resolved versions and local fork contracts before changing APIs.

- Reuse the existing Endpoint and protocol router. Keep ALPN identifiers and stream
  framing stable; bound message lengths before allocating or deserializing.
- QUIC authenticates a peer identity. Bind that identity to the authenticated player,
  seat, game and session; transport identity alone does not authorize a chess move.
- Preserve direct connectivity and relay fallback. Diagnose reachability using
  endpoint logs/metrics before changing discovery, relay or firewall settings.
- Bound per-peer streams, subscriptions, buffers and reconnects. Keep cancellation
  and endpoint shutdown tied to the session, including error and half-close paths.
- Gossip delivery is not durable application acknowledgment. Preserve version
  deduplication, pending writes and application-confirmed cursors across reconnects.
- In the local h3 adapters, preserve request/response body streaming, finish/reset
  semantics and authenticated peer metadata. Reuse connections without reusing a
  canceled or failed request's state.

Use existing lifecycle and peer integration tests. Test two real local endpoints,
disconnect/reconnect, cancellation, duplicate delivery and oversized frames as applicable.

Sources: [Iroh Rust API](https://docs.rs/iroh/latest/iroh/),
[QUIC](https://docs.iroh.computer/protocols/using-quic),
[protocols](https://docs.iroh.computer/concepts/protocols),
[gossip](https://docs.iroh.computer/connecting/gossip),
[relays](https://docs.iroh.computer/add-a-relay),
[h3](https://docs.rs/h3/latest/h3/).
