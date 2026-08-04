# zarathustra_net — networking stack

The Braid-HTTP (209) protocol stack and Iroh QUIC transports that power XFChess's
live game subscriptions, tournament document sync, and P2P relay. XFChess is
server-authoritative throughout: the relay server (and, for staked games, the Solana
program) is the single source of truth; clients subscribe to update streams rather
than merging concurrent edits.

## Layering

```
braid_chess (typed chess messages, pub/sub API)
    │
braid-core (stable facade: protocol types + runtime traits)
    │
braid-http (HTTP-209 codec + reqwest/WASM client)          xfchess-braid-server (Axum-native 209 server / fan-out hub)
    │
braid-iroh (Braid over Iroh QUIC)  ── iroh-h3 / iroh-h3-axum / iroh-h3-client (HTTP/3 over Iroh) ── iroh-gossip (broadcast)
```

## Crates

| Crate | Role |
|-------|------|
| [`braid_chess/`](braid_chess/) | Chess application layer: maps games onto Braid resources (`/game/{id}/moves`, `/clock`, `/engine`, `/chat`) with a typed publish/subscribe API |
| [`braid-core/`](braid-core/) | Thin facade over `braid-http`: one stable import path for protocol types, plus runtime-abstraction traits (`BraidRuntime`/`BraidNetwork`/`BraidStorage`) |
| [`braid-http/`](braid-http/) | The HTTP-209 protocol implementation: wire codec and the reqwest (native) / WASM (browser) client backends |
| [`braid-iroh/`](braid-iroh/) | Braid protocol tunneled over Iroh QUIC connections for P2P relay |
| [`xfchess-braid-server/`](xfchess-braid-server/) | Axum-native HTTP-209 server: subscription registry and update fan-out, embedded by the backend |
| [`iroh-h3/`](iroh-h3/), [`iroh-h3-axum/`](iroh-h3-axum/), [`iroh-h3-client/`](iroh-h3-client/) | HTTP/3 over Iroh: core layer, Axum server integration, and client |
| [`iroh-gossip/`](iroh-gossip/) | Gossip broadcast over Iroh, used by the backend relay |

`iroh-gossip` (upstream [n0-computer/iroh-gossip](https://github.com/n0-computer/iroh-gossip))
and the three `iroh-h3*` crates (upstream [Le-Maz/iroh-h3](https://github.com/Le-Maz/iroh-h3))
are vendored third-party code — keep their docs and comments as upstream wrote them.

## Channel roles

Braid HTTP-209 subscriptions carry **state sync** (move logs, clocks, tournament
documents via JSON Patch). WebSockets carry **auth and signaling** (see
`src/multiplayer/`). Keep the two distinct when adding features.

Protocol provenance: Rust port of the [braid.org](https://braid.org) JavaScript
reference (MIT OR Apache-2.0) — see `xfchess-braid-server/ATTRIBUTION.md`.
