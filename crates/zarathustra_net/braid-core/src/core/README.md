# `core` module

The internals of [`braid-core`](../../README.md). This module holds the runtime-abstraction
traits and the local error type, then re-exports the protocol vocabulary from
[`braid-http`](../../../braid-http).

The module is intentionally small: XFChess is server-authoritative, so there is no
client-side conflict resolution, merge engine, or middleware here — just the three
runtime traits, the error type, and protocol re-exports.

## Contents

### `traits.rs` — runtime abstractions

Three traits decouple the protocol from its execution environment, so identical logic runs
on a Tokio server or in a WASM browser build:

| Trait | Purpose |
|-------|---------|
| `BraidRuntime` | `spawn` a future, `sleep`, read `now_ms` |
| `BraidNetwork` | `fetch(url, req)` and `subscribe(url, req)` |
| `BraidStorage` | `put` / `get` / `delete` / `list_keys` |

### `error.rs`

`BraidError` and the crate `Result` alias for the core surface.

### `mod.rs`

Top-level re-exports of the most commonly used protocol types
(`BraidRequest`, `BraidResponse`, `Patch`, `Update`, `Version`) sourced from `braid-http`.

## Integration flow (XFChess, server-authoritative)

1. A consumer (`braid_chess`, `braid-iroh`) builds a `BraidRequest`.
2. It is dispatched through a `BraidNetwork` implementation (`braid-http`'s reqwest or WASM
   backend).
3. On `PUT`, the **server** ([`xfchess-braid-server`](../../../xfchess-braid-server)) is the
   single source of truth — it appends to a log or applies a JSON Patch and fans the update
   out to subscribers. There is no client-side conflict resolver.
4. Subscribers receive the new `Update` (snapshot or patch) over their transport channel.
