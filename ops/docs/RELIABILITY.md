# Reliability and recovery objectives

These are planning targets, not a published SLA or measured production guarantee.

| Flow | Initial target |
| --- | --- |
| API reads | 99.5% success over 30 days; p95 below 300 ms, p99 below 800 ms |
| Authentication | 99.5% success; p95 below 1.5 seconds |
| Wagered moves | 99% accepted; p95 below one second on the ER |
| Settlement | 99% completed within two minutes of a committed terminal result |
| Tournament operations | 99% completed within one minute of the trigger |

HTTP/RPC counters and histograms measure instrumented boundaries. Move acceptance,
settled balances and tournament completion require chain/workflow evidence.
Existing alerts are thresholds, not a complete error-budget burn policy.
Establish baselines before treating these targets as gates.

`/health` reports process/build identity; `/readyz` checks database readiness;
`/health/detailed` exposes dependency/resources. Private `/metrics` serves
Prometheus. Grafana/Alertmanager remain on loopback; delivery needs a configured
receiver. See [monitoring](../README.md#monitoring-and-backups).

A release restarts a single service. Graceful shutdown and rollback help recovery
but do not provide zero-downtime rolling deployment. Migrations are not reversed.

Daily encrypted SQLite backup recovers to the last successful snapshot, potentially
over 24 hours old after jitter or failed runs. A 30-minute database recovery is an
unmeasured target until a real drill. The timer excludes JSONL/archive data,
environment secrets and offline keys. Chain state cannot reconstruct all local
sessions or tournament records.

Follow [recovery](ROLLBACK_GUIDE.md) with explicit encrypted objects and compatible
vault secrets. A new VPS needs provisioning, TLS/env setup, a verified release
and recovered data before switching traffic. See [runbooks](../../docs/runbooks/README.md).
