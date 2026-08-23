# xfchess-braid-server

The **server side** of Braid-HTTP for XFChess: an Axum-native implementation of the
`209 Subscription` protocol that streams live state — primarily **tournament** data
(standings, pairings, roster, meta) and game move logs — to browsers and game clients
**without polling**.

This is the relay/fan-out hub the clients ([`braid_chess`](../braid_chess),
[`braid-http`](../braid-http)) subscribe to. Part of the XFChess Rust Braid implementation
(a port of the braid.org JavaScript reference).

## The problem it solves

A tournament dashboard or a spectator view needs to reflect server state in real time. Polling
is wasteful and laggy; a bespoke WebSocket protocol per view is a lot of plumbing. This crate
exposes any registered resource as a Braid `209` stream: a client `GET`s it once with
`Subscribe: true` and receives a **snapshot followed by a live tail** of updates. When
the Swiss orchestrator advances a round, it patches the standings resource and every connected
bracket updates instantly.

## Resource model — two shapes

The [`ResourceHub`] maps a path to one of two resource kinds and fans updates out to all
subscribers via a `tokio::broadcast` channel:

| Kind | Backing | Used for | Update op |
|------|---------|----------|-----------|
| [`PatchedDoc`] | a JSON document | tournament `meta`, `standings`, `roster`, `schedule-status`, `pairings/{round}` | `patch` (RFC 6902 JSON Patch) or `replace` |
| [`AppendLog`] | an ordered list | tournament `results`, game move sequences | `append` |

There is **no CRDT/OT merge** — the hub is the single source of truth (server-authoritative).
`PatchedDoc` applies JSON Patches in order; `AppendLog` is append-only. This is the deliberate
XFChess simplification of the upstream braid.org engine (see `ATTRIBUTION.md`).

Both shapes travel as a JSON **body**, so the media type is what distinguishes them:
`application/json` for a snapshot, `application/json-patch+json` for a patch document.
A receiver that ignores it will mistake a patch for the state.

## One write, two transports

A hub write fans out twice:

1. to local subscribers, over each resource's `tokio::broadcast` channel — what the
   `209` handler streams to browsers and game clients; and
2. to the **gossip sink**, if one is installed with [`ResourceHub::set_gossip_sink`] —
   the same update, carried to P2P peers.

So a fact is written *once* and both transports carry identical versioned bytes. The
backend installs the sink in `server.rs`, routing each update to its tournament's gossip
topic; `resource::protocol::encode_for_gossip` tags it with the resource path, since a
gossip receiver has no request line to read the path from.

This replaced a second, parallel publication path: `SwissService` used to broadcast
tagged-JSON `SwissMessage`s over gossip beside each hub write — no version, no parents,
no catch-up. (Those particular broadcasts never actually fired: they were guarded on a
`gossip` field whose only setter was never called. An unfinished SQLite replay table sat
beside them for the late-joiner gap; nothing ever wrote to it either.) Subscribe-replay
covers that case by construction.

## API

```rust,no_run
use xfchess_braid_server::{ResourceHub, braid_router, bridge};

#[tokio::main]
async fn main() {
    let hub = ResourceHub::new();

    // Register the standard resources for tournament 42.
    hub.ensure_tournament(42);
    hub.ensure_pairings(42, 1);

    // Mount the subscribe routes on an Axum app:
    //   app.nest("/braid", braid_router(hub.clone()))
    let _router = braid_router(hub.clone());

    // Push an update — fans out to all subscribers of tournament/42/roster:
    bridge::push_roster(&hub, 42, &["wallet1".into(), "wallet2".into()]);
}
```

| Item | Purpose |
|------|---------|
| `ResourceHub` | Registry of all live resources + their subscriber channels. `register_doc`/`register_log`, `patch`/`replace`/`append`, `subscribe`, `current_json`, and `ensure_tournament`/`ensure_pairings` helpers |
| `PatchedDoc` / `AppendLog` | The two resource backings |
| `braid_router(hub)` | Axum `Router` serving `GET /*res` via `resource::subscribe::get_resource` (with permissive CORS) |
| `bridge` | Helpers that translate XFChess domain events (roster/standings/pairings changes) into hub updates |

## Module map

| Module | Contents |
|--------|----------|
| `hub.rs` | `ResourceHub` — registration, mutation, fan-out |
| `resource/store.rs` | `PatchedDoc` and `AppendLog` (snapshot + `broadcast` tail) |
| `resource/subscribe.rs` | The Axum handler that emits the `209` stream |
| `resource/protocol.rs` | `BraidUpdate` wire shape |
| `bridge.rs` | Domain-event → resource-update helpers |

## Role in XFChess

```
backend (Axum)
  └── SigningState { braid_hub: Arc<ResourceHub> }
        ├── Swiss orchestrator  ── patch ──► tournament/{id}/standings|pairings|roster
        └── braid_router  mounted at /braid/*  ◄── browsers & clients subscribe (209)
```

Mounted from `backend/src/signing/` and driven by the Swiss tournament service. Game clients
reach it through `braid_chess` / `braid-http`; web spectators subscribe directly over HTTP.

## Provenance & license

See [`ATTRIBUTION.md`](./ATTRIBUTION.md). Implements the Braid-HTTP 209 protocol (© the Braid
working group, MIT OR Apache-2.0); the XFChess server adaptation (tournament resources, JSON
Patch in place of CRDT, append-log move backend, Solana session-key auth, iroh-gossip bridge)
is original work in this crate.
