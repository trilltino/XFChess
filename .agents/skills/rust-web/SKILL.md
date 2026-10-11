---
name: rust-web
description: Implement or debug XFChess async services using Tokio, Axum, Tower, reqwest, tracing, JWT and Argon2. Use for backend handlers, middleware, streaming, task ownership and shutdown.
metadata:
  provenance: docs-derived
---

# Async Rust services

Read the relevant Cargo manifest and lockfile before choosing APIs. Start at
`backend/src/infrastructure/router.rs`, `backend/src/signing/routes/`, or
`tauri/src/wallet/http.rs`; reuse the existing server and runtime.

- Keep HTTP extraction, domain operations and persistence in their existing modules.
  Use typed `State` and explicit response/error types; put body-consuming extractors last.
- Axum 0.8 route captures use `{id}` and `{*path}`. Check middleware ordering and
  authorization on the actual nested route, including WebSocket upgrades and SSE.
- Reuse reqwest clients and database pools. Bound request bodies, concurrency,
  queues and deadlines. Respect 429/retry-after without replaying mutations blindly.
- Keep locks short and release them before network calls or callbacks. Move blocking
  work off runtime workers; bound CPU-heavy Argon2 and engine analysis separately.
- Give background tasks a lifecycle owner. Propagate cancellation, stop producers,
  close channels and join tasks during shutdown. Dropping a JoinHandle detaches its task.
- Verify JWT signature, configured algorithm, expiry and applicable claims. Preserve
  existing Argon2 parameters and authentication middleware. Redact tokens and request secrets.
- Treat network delivery, database commit and game/application acknowledgment as
  distinct events. Reconcile ambiguous transaction submissions before retrying.

Use existing in-process backend tests or wiremock fixtures. Exercise rejected input,
disconnects, deadline expiry and shutdown when the change affects those paths.

Sources: [Tokio tutorial](https://tokio.rs/tokio/tutorial),
[shutdown](https://tokio.rs/tokio/topics/shutdown),
[Axum](https://docs.rs/axum/latest/axum/),
[Tower HTTP](https://docs.rs/tower-http/latest/tower_http/),
[reqwest](https://docs.rs/reqwest/latest/reqwest/),
[tracing](https://docs.rs/tracing/latest/tracing/),
[jsonwebtoken](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/),
[Argon2](https://docs.rs/argon2/latest/argon2/).
