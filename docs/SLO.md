# Service Levels & Reliability Targets — XFChess

What "up" means, the targets we hold ourselves to, and the error budget that governs
whether we ship features or fix reliability. Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-H.

> Targets are **initial** (pre-scale, single VPS + devnet). Revisit once we have real
> traffic and the load test (WS-C) gives measured p95/p99.

## SLIs — what we measure

Measured from Prometheus (`/metrics`) RED metrics per route + worker.

| Flow | SLI (success definition) | Where |
|---|---|---|
| Web/API reads (profile, leaderboard, tournament) | HTTP 2xx within budget | nginx + backend metrics |
| Auth (register/login/sync) | 2xx, no 5xx | `/api/auth/*` |
| Move recording (ER) | move landed on ER within budget | game client + `record_move` telemetry |
| Wager settlement | `finalize_game` submitted after result committed | `settlement_worker` metrics |
| Tournament ops (pair/advance/prizes) | scheduled action completes | `tournament_scheduler` metrics |
| Email (confirmation/waitlist) | send accepted or queued | mailer logs |

## SLOs — targets (rolling 30 days)

| Flow | Availability | Latency target | Notes |
|---|---|---|---|
| API reads | 99.5% | p95 < 300ms, p99 < 800ms | cacheable (WS-C) |
| Auth | 99.5% | p95 < 1.5s | includes 1 RPC read |
| Move recording | 99.0% | p95 < 1s (ER sub-second) | depends on MagicBlock ER |
| Settlement | 99.0%, ≤ 2 min after result | — | worker scans every 30s |
| Tournament ops | 99.0% | ≤ 1 min after trigger | — |
| Email | 98% | best-effort, async | non-blocking; stored first |

**Latency percentiles:** track p50 / p95 / p99 on every flow. p99 is the SLO gate.

## Error budget & burn policy

- **Budget** = 1 − SLO. e.g. API reads 99.5% → **0.5% / 30d ≈ 3.6h** of allowed failure.
- **Burn policy:**
  - Budget > 50% remaining → ship features normally.
  - Budget 10–50% → prioritize reliability fixes; risky changes need review.
  - Budget < 10% or exhausted → **feature freeze**; only reliability/bugfix deploys until recovered.
- Fast-burn alert: >2% of budget in 1h → page. Slow-burn: >10% in 6h → ticket.

## SLA (external commitment)

None published yet (pre-launch). When wagering goes live, commit **99.5%** on API +
settlement, with the maintenance-window carve-out below.

## Maintenance windows

Deploys are zero-downtime (graceful shutdown + rolling; WS-A/G) so no scheduled window is
normally needed. Migrations that can't be expand-contract get a pre-announced window
(low-traffic UTC night). Chain/ER provider maintenance is outside our SLA (documented in
[VENDORS.md](VENDORS.md) once created).

## Ownership & notification

- SLO owner: **@trilltino** (solo). Alert routing via Alertmanager → (configure a real
  channel: email/Telegram). Every SLO-linked alert must map to a runbook in
  [`runbooks/`](runbooks/).

## Health surface

- `GET /health` — liveness (process up) + `version` + `git_sha` (which commit is running).
- `GET /readyz` — readiness (DB reachable) → 503 if not; used by deploy smoke test.
- `GET /health/detailed` — DB, Solana RPC, fee-payer pool, disk, memory.
- `GET /metrics` — Prometheus RED + worker/anti-cheat counters.
