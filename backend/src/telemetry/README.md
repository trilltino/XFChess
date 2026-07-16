# backend/src/telemetry

Observability: Prometheus metrics, structured logging, and per-request tracing.
Scraped at `GET /metrics`; dashboards live in [deploy/monitoring/](../../../deploy/monitoring/).

## Key files

| File | Contents |
|------|----------|
| [metrics.rs](metrics.rs) | Prometheus registry + API/Solana/game-session metrics |
| [worker_metrics.rs](worker_metrics.rs) | Background-worker health gauges (see [../tasks/](../tasks/README.md)) |
| [middleware.rs](middleware.rs) | Axum layer: request timing + `request_id` span on every request |
| [logging.rs](logging.rs) | Structured logging setup (`LOG_FORMAT=json` → one JSON object per line) |

## Invariants

- New endpoints get metrics automatically via the middleware layer; only add explicit
  counters for domain events (settlements, disputes), not per-route plumbing.
- Log with the `request_id` span intact — it's how production incidents are traced
  across the runbooks in [docs/runbooks/](../../../docs/runbooks/).
