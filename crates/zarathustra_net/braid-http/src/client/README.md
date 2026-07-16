# Braid-HTTP Client Module (`client`)

The `client` module provides the high-level API for interacting with Braid-HTTP servers, supporting both native (Tokio/reqwest) and WebAssembly (browser fetch) runtimes.

## Key Sub-modules

### Network Implementations (`native_network.rs`, `wasm_network.rs`)
Implements the `BraidNetwork` trait for different backends.
- **Native**: `reqwest` + `tokio`; supports pooling, proxies, and streaming.
- **WASM**: bridges to the browser `fetch` API via `js-sys` and `wasm-bindgen-futures`.

### Subscription Management (`subscription.rs`, `multiplex.rs`)
Handles long-lived Braid subscriptions. It manages the connection lifecycle, heartbeats, and ensures that incoming updates are correctly parsed and delivered to the application via async channels.

### Request/Response Engine (`fetch.rs`, `parser.rs`)
- **`fetch.rs`**: The low-level execution engine that constructs `BraidRequest` objects and dispatches them through the selected network backend.
- **`parser.rs`**: Parses Braid-HTTP response bodies — full state snapshots and the Braid patch format (including multipart/related and SFV-encoded patches).

### Reliability (`retry.rs`)
Retry policies with exponential backoff so subscriptions survive intermittent network failures.

### Headers and Utilities (`headers.rs`, `utils.rs`)
Helpers for extracting and validating Braid-specific headers (Version, Parents, Merge-Type) from standard HTTP responses.

## Usage Pattern

Most users will interact with this module through the `BraidClient` re-exported at the crate root.

```rust,no_run
use braid_http::{BraidClient, types::BraidRequest};

let client = BraidClient::new()?;                       // no args; returns Result
let mut sub = client
    .subscribe("http://host/my-resource", BraidRequest::new().subscribe())
    .await?;
while let Some(update) = sub.next().await { /* handle Result<Update> */ }
```
