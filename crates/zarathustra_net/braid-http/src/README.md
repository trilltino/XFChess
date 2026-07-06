# `braid-http/src` module map

Implementation of the Braid-HTTP `209` protocol. See the [crate README](../README.md) for the
protocol overview, wire format, and client usage.

| Module | Responsibility |
|--------|----------------|
| `types/` | Data model: `Version`, `Patch`, `Update`, `BraidRequest`/`BraidResponse`, `content_range` |
| `protocol/` | Wire format: SFV `headers`/`formatter`, `parser` (209-stream state machine), `multiplex` (update framing), `constants` (status codes, header names, merge types) |
| `client/` | `BraidClient`: `fetch` (one-shot), `subscription` (long-lived + heartbeat), `retry`, and the `native_network` (reqwest) / `wasm_network` (browser fetch) transport backends |
| `traits.rs` | The `BraidNetwork` abstraction the client dispatches through |
| `error.rs` | `BraidError` (incl. `Timeout`, `SubscriptionClosed`) and `Result` |
