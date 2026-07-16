# backend/src/infrastructure

Wiring layer between `signing_server.rs` and the domain code: database pools, router
assembly, background-task spawning, admin auth middleware, and the ngrok dev tunnel.
No business logic lives here.

## Key files

| File | Contents |
|------|----------|
| [database.rs](database.rs) | `initialize_pools` (SQLite, WAL) + `run_migrations` over [../../migrations/](../../migrations/) |
| [router.rs](router.rs) | `build_app_router` — merges every `signing/routes/*` router, applies CORS + telemetry middleware |
| [tasks.rs](tasks.rs) | `spawn_background_tasks` — settlement worker, tournament scheduler, anticheat queue, archiver |
| [auth_middleware.rs](auth_middleware.rs) | `require_api_key` — guards admin routers with `ADMIN_API_KEY` |
| [ngrok.rs](ngrok.rs) | Optional dev tunnel so wallets/webhooks can reach a local server |

## Example

```rust
// router.rs — every API area exports a router; this is the single merge point
use crate::infrastructure::auth_middleware::require_api_key;
use crate::signing::routes::admin::admin_routes;
use crate::signing::routes::matchmaking::matchmaking_routes;

pub fn build_app_router(signing_state: AppState) -> Router<AppState> {
    // …merge signing, tournament, matchmaking, puzzle, kyc, mailer routers;
    // admin_routes are wrapped in require_api_key
}
```

## Invariants

- New API areas get a `*_routes()` function in `signing/routes/` and one merge line in
  [router.rs](router.rs) — don't build routers elsewhere.
- Anything admin-facing must be layered behind `require_api_key`.
- CORS policy is currently permissive; locking it to `ALLOWED_ORIGINS` is tracked as
  R4 in [deploy/docs/E2E_REMEDIATION.md](../../../deploy/docs/E2E_REMEDIATION.md).
