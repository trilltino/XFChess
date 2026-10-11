# Multiplayer networking

| Crate | Responsibility |
| --- | --- |
| `braid_chess` | Chess messages, resource paths, HTTP publisher/subscriber, tournament events |
| `braid-http` | Braid headers, update framing, native HTTP client and subscription lifecycle |
| `braid-iroh` | Iroh discovery/gossip, bounded event history, persistence and optional TCP bridge |
| `iroh-h3` | Zero-copy Iroh QUIC adapter for the `h3` HTTP/3 implementation |
| `iroh-h3-axum` | Serves an Axum router over Iroh HTTP/3 |
| `iroh-h3-client` | Reuses Iroh HTTP/3 connections and streams request/response bodies |
| `xfchess-braid-server` | Backend document/log hub and HTTP 209 subscriptions |

`braid_chess`, `braid-iroh` and the backend hub share the types and wire codec in
`braid-http`. `braid-core` was a redundant facade and has been removed.
Iroh gossip comes from the upstream Cargo dependency, not a vendored directory.

The desktop bridge serves its own resources directly through the shared Axum
router. Requests to another node use the three HTTP/3 crates. Backend tournament
subscriptions send a snapshot followed by JSON Patch updates; a lagging subscriber
receives a fresh snapshot. Game history subscriptions replay retained events and
close on lag so consumers can reconnect.
Optional HTTP retries resume from delivered versions. The bridge honors `Parents`
and returns HTTP 410 when the requested history is unavailable. Upstream research
and implementation decisions are recorded in [UPSTREAM.md](UPSTREAM.md).

The Iroh event store retains 500 updates per resource. It is a replay cache, not
an archive or a chess rules validator. Consumers remain responsible for validating
moves, authorization, causal ordering and reconnect policy. Production nodes use
`DiscoveryConfig::Real`; local tests use isolated discovery without public relays.
Pass a persisted secret key when a stable node identity is required. The native
Braid HTTP client has no browser transport.

Validation:

```sh
cargo test -p braid-http -p braid-iroh -p braid_chess -p iroh-h3 -p iroh-h3-axum -p iroh-h3-client -p xfchess-braid-server --all-targets --features braid-iroh/proxy,iroh-h3-client/json
```

HTTP/3 adapters are maintained forks of [Le-Maz/iroh-h3](https://github.com/Le-Maz/iroh-h3).
Braid provenance is recorded in [ATTRIBUTION.md](xfchess-braid-server/ATTRIBUTION.md).
