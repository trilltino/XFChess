# backend/src/tasks

Long-running background workers, spawned at startup by
[../infrastructure/tasks.rs](../infrastructure/tasks.rs) (`spawn_background_tasks`).
They make the platform self-driving: games settle and prizes pay out without any
client involvement.

## Workers

| File | Worker |
|------|--------|
| [settlement_worker.rs](settlement_worker.rs) | Scans active sessions every 30s, reads the `Game` PDA, auto-submits `finalize_game` once a result is committed |
| [tournament_scheduler.rs](tournament_scheduler.rs) | Auto-starts scheduled tournaments; `spawn_prize_distributor` cranks the permissionless `distribute_tournament_prizes` instruction |
| [matchmaking.rs](matchmaking.rs) | Elo-based pairing queue |
| [anticheat_worker.rs](anticheat_worker.rs) | Consumes the anticheat queue ([queue.rs](queue.rs), migration 019) and runs engine-similarity analysis |
| [fee_claimer.rs](fee_claimer.rs) | Claims accumulated platform fees from the on-chain vault |
| [archiver.rs](archiver.rs) | Moves finished games into the archive tables (compressed PGN) |

## Invariants

- Workers must be **idempotent** — they re-scan on every tick and may observe the
  same state twice (e.g. settlement double-checks the on-chain status before
  submitting).
- New workers are registered in `spawn_background_tasks`, not spawned ad hoc from
  route handlers.
- Worker health is exported via [../telemetry/worker_metrics.rs](../telemetry/worker_metrics.rs).
