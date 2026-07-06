# braid-http

A Rust implementation of the **Braid-HTTP** protocol (HTTP `209 Subscription`) — the
streaming, versioned synchronization extension to HTTP. This crate is a Rust port of
the JavaScript reference implementation at [braid.org](https://braid.org)
([github.com/braid-org](https://github.com/braid-org)); the protocol itself is specified
by the Braid working group in [`draft-toomim-httpbis-braid-http`](https://datatracker.ietf.org/doc/html/draft-toomim-httpbis-braid-http).

It is the protocol core of the XFChess `zarathustra_net` networking stack: every other
`braid-*` crate is built on the types and client defined here.

## What Braid-HTTP is

Plain HTTP gives you a *snapshot* of a resource on each `GET`. Braid-HTTP turns a
resource into a **live, versioned stream**: one `GET` with `Subscribe: keep-alive`
returns the current state *and then stays open*, pushing each subsequent change as an
incremental update. Writers `PUT` updates tagged with a `Version` and its `Parents`,
forming a version DAG (the same shape git uses for commits).

This buys three things a raw socket does not give you for free:

1. **Snapshot + tail on subscribe** — a late or reconnecting client receives current
   state followed by the live update feed, with no separate "catch-up" path.
2. **Versioning / resumption** — every update is addressable; a client can declare the
   version it last saw and resume from there.
3. **A uniform resource model** — *any* JSON resource becomes a feed. The same mechanism
   serves chess moves, clocks, tournament standings, and chat.

## Wire protocol

### Status codes (`protocol::constants::status`)

| Code | Meaning |
|------|---------|
| `200 OK` | Ordinary one-shot response |
| `206 Partial Content` | Range-scoped patch (`Content-Range`) |
| **`209 Subscription`** | Streaming subscribe response — connection stays open, updates follow |
| `293 Merge Conflict` | Concurrent versions could not be reconciled |
| `410 Gone` | Requested history has been dropped |
| `416 Range Not Satisfiable` | Invalid patch range |

### Headers (`protocol::constants::headers`)

| Header | Role |
|--------|------|
| `Subscribe` | Request a subscription (`keep-alive`) and declare heartbeat interval |
| `Version` | The version id this update introduces |
| `Parents` | The version(s) this update builds on (the DAG edges) |
| `Merge-Type` | How to reconcile concurrent writes (`replace`, `simpleton`, `diamond`) |
| `Patch-Type` | The patch encoding when an update is a partial change |
| `Content-Range` | The byte/unit range a patch applies to |

Header values use **Structured Field Values** (SFV, RFC 8941). A long-lived `209`
stream frames successive updates as multipart bodies; `protocol::multiplex` manages the
boundaries and associates each update's metadata with its body.

## Module map

| Module | Responsibility |
|--------|----------------|
| `types/` | The data model: `Version` (string or integer id), `Patch`, `Update` (a received change: snapshot body or patch list), `BraidRequest` / `BraidResponse` builders, `content_range` |
| `protocol/` | Wire format: `headers`/`formatter` (parse & emit SFV headers), `parser` (state-machine decode of a `209` stream), `multiplex` (update framing), `constants` (status codes, header names, merge types) |
| `client/` | High-level API — see below |
| `error.rs` | `BraidError` (incl. `Timeout`, `SubscriptionClosed`) and `Result` |
| `traits.rs` | The `BraidNetwork` abstraction the client dispatches through |

### `client/` — the `BraidClient`

The portable client API, re-exported at the crate root.

- **Transport backends** (`BraidNetwork` impls): `native_network.rs` is powered by
  `reqwest` (pooling, streaming) on Tokio; `wasm_network.rs` bridges to the browser
  `fetch` API via `wasm-bindgen-futures`. The protocol logic is identical across both.
- **`fetch.rs`** — one-shot requests (`GET`/`PUT`/`POST`/…), used for publishing updates.
- **`subscription.rs`** — long-lived subscriptions. A `Subscription` yields each `Update`
  and enforces **heartbeat liveness**: if no data and no heartbeat arrive within
  `1.2 × interval + 3s`, it surfaces `BraidError::Timeout` so the caller can reconnect.
  Uses a native/wasm `Instant` shim so the same timing logic runs in-browser.
- **`retry.rs`** — reconnect/backoff policy for resilient subscriptions.
- **`multiplex.rs`**, **`headers.rs`**, **`utils.rs`** — stream demux and header helpers.

```rust,no_run
use braid_http::{BraidClient, types::BraidRequest};

#[tokio::main]
async fn main() -> braid_http::Result<()> {
    let client = BraidClient::new()?;

    // Publish: PUT an update with explicit version + parent.
    let put = BraidRequest::new()
        .with_method("PUT")
        .with_body(b"{\"hello\":\"world\"}".to_vec())
        .with_content_type("application/json")
        .with_version("v2".into())
        .with_parent("v1".into())
        .with_merge_type("replace");
    client.fetch("http://localhost:3000/doc", put).await?;

    // Subscribe: snapshot + live tail.
    let mut sub = client.subscribe("http://localhost:3000/doc",
                                   BraidRequest::new().subscribe()).await?;
    while let Some(update) = sub.next().await {
        match update {
            Ok(u)  => println!("update: {:?}", u.body_str()),
            Err(e) => { eprintln!("stream error: {e}"); break; }
        }
    }
    Ok(())
}
```

## Features

| Feature | Effect |
|---------|--------|
| `native` *(default)* | `reqwest`/Tokio transport |
| `wasm` | Browser `fetch` transport (`js-sys`, `wasm-bindgen-futures`, `gloo-timers`) |
| `fuzzing` | Exposes the parser fuzzer harness |

## Role in XFChess

```
braid_chess ──┐
braid-iroh ───┼──► braid-core ──► braid-http  ◄── you are here
xfchess-braid-server (server side, own 209 impl)
```

`braid-core` re-exports this crate's `Version`/`Update`/`BraidClient` as the thin surface
the rest of the workspace consumes. `braid_chess` builds typed chess messages on top of the
client; `braid-iroh` tunnels the same protocol over QUIC.

## Provenance

Rust implementation by the XFChess author, ported from the braid.org JavaScript reference.
Protocol design © the Braid working group (IETF draft, MIT OR Apache-2.0). See
`xfchess-braid-server/ATTRIBUTION.md` for the workspace-wide attribution and license note.
