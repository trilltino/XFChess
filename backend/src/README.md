# backend/src

Source tree of the backend server. Component overview and API examples live in
[../README.md](../README.md); this file is the directory map.

## Layout

| Path | Contents |
|------|----------|
| [signing_server.rs](signing_server.rs) | `signing-server` binary — **the** API server entry point |
| [main.rs](main.rs) | `backend` binary — stub that points users at `signing-server` |
| [lib.rs](lib.rs) | Library root so binaries and tests share the module tree |
| [error.rs](error.rs) | Unified error type for all handlers |
| [bin/](bin/README.md) | Auxiliary binaries: `vps_admin`, `tournament_admin`, `import_puzzles` |
| [signing/](signing/README.md) | Core domain: auth, transaction building, routes, compliance, relay, tournament storage |
| [infrastructure/](infrastructure/README.md) | Router assembly, DB pool init, auth middleware, task spawning, ngrok dev tunnel |
| [db/](db/README.md) | SQLite persistence: game archive, moves feed, sessions |
| [tasks/](tasks/README.md) | Background workers (settlement, prizes, matchmaking, anticheat, archiver) |
| [telemetry/](telemetry/README.md) | Prometheus metrics, structured logging, HTTP middleware |

## Startup path

```rust
// signing_server.rs
use backend::infrastructure::{initialize_pools, run_migrations, spawn_background_tasks};
// initialize_pools + run_migrations  → SQLite pools, WAL, numbered migrations
// build_app_router(signing_state)    → merges signing/routes/* routers + middleware
// spawn_background_tasks             → settlement worker, tournament scheduler, anticheat queue
// LOG_FORMAT=json switches to structured one-line-per-object logs with request_id
```

## Conventions

- Handlers live in `signing/routes/`, one file per API area, each exporting a
  `*_routes()` router that [infrastructure/router.rs](infrastructure/router.rs) merges.
- All errors flow through [error.rs](error.rs) — handlers return `Result<_, AppError>`
  rather than ad-hoc status codes.
- Admin-only routers are wrapped with `require_api_key`
  ([infrastructure/auth_middleware.rs](infrastructure/auth_middleware.rs)).
