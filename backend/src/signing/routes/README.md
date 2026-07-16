# backend/src/signing/routes

All HTTP handlers, one file per API area. Each file exports a `*_routes()` router that
[../../infrastructure/router.rs](../../infrastructure/router.rs) merges into the app.

## Route areas

| File | API area |
|------|----------|
| [main.rs](main.rs) | Core game flow: `/session/*`, `/move/record`, `/game/*`, profiles, stats |
| [auth.rs](auth.rs) | Wallet-first auth: challenge/verify → JWT |
| [matchmaking/](matchmaking/) | Queue join/leave + state ([handlers.rs](matchmaking/handlers.rs), [state.rs](matchmaking/state.rs)) |
| [tournament.rs](tournament.rs) | Tournament CRUD, registration, standings |
| [admin.rs](admin.rs) | Admin ops — wrapped in `require_api_key` |
| [dispute.rs](dispute.rs) | Player dispute submission + admin resolution |
| [kyc.rs](kyc.rs) / [identity.rs](identity.rs) | KYC flow and encrypted identity vault |
| [external_elo.rs](external_elo.rs) / [lichess_oauth.rs](lichess_oauth.rs) | Linked external ratings (Lichess OAuth) |
| [puzzle.rs](puzzle.rs) | Tactics puzzles (+ admin import routes) |
| [rates.rs](rates.rs) | SOL/USD wager-tier pricing (Pyth) |
| [history.rs](history.rs) / [archive.rs](archive.rs) | Game history + archived PGN retrieval |
| [chat.rs](chat.rs) | In-game chat |
| [global_session.rs](global_session.rs) | Wallet-wide session keys (one popup ever) |
| [relayer.rs](relayer.rs) | Fee-relayer endpoints |
| [wallet.rs](wallet.rs) | Wallet balance/info |
| [mailer.rs](mailer.rs) | Signup/waitlist PDF mailer |
| [anticheat.rs](anticheat.rs) | Anticheat telemetry ingest |
| [debug.rs](debug.rs) | `GET /api/debug/transaction/:signature` inspector |

## Conventions

- Handlers return `Result<_, AppError>` ([../../error.rs](../../error.rs)) and take
  `State<AppState>`.
- Wager-building routes call [../cacf/](../cacf/) checks first.
- Admin-only routers are registered behind `require_api_key` in the router merge —
  never guard inside the handler.
