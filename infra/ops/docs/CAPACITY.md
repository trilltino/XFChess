# Capacity and scaling

Measure the deployed workload before choosing a server, provider plan or database.
There is no sustained-QPS claim in this guide.

| Boundary | Current mechanism | Measure |
| --- | --- | --- |
| Rates | `RateCache`: 60-second freshness, stale-on-fetch-error | Upstream requests and stale duration |
| Player ratings | Wallet-keyed `EloCache`, caller-configured TTL/invalidation | Cache misses and RPC latency |
| RPC | Primary/fallback reads, timeouts and circuit breaker | Throttling, retries, confirmations and billed calls/traffic |
| SQLite | WAL databases and serialized writes | Busy errors, latency, DB/WAL growth and recovery time |
| Anti-cheat | Bounded analysis workers | CPU/RAM, backlog and time to verdict |
| Peer/subscription delivery | Iroh/Braid and durable event log | Reconnect time, subscriptions, replay bytes and relay traffic |

[cu_benchmark](../../tools/cu_benchmark/) separates offline estimates,
backend workflows, live chain measurement and RPC probes. Saved observations can
be repriced; missing measurements/prices remain unknown. Start with
[workloads](../../tools/cu_benchmark/docs/system.md) and
[pricing](../../tools/cu_benchmark/docs/pricing.md). Separate rent, refundable
balances, RPC/network usage and fixed VPS costs.

Record commit, host resources, region, provider, cluster, workload, concurrency,
duration, completed/failed games and p50/p95/p99. A lightweight RPC read probe
does not establish capacity for 100 complete games or a tournament.

Scale the measured constraint: worker capacity for analysis backlog, RPC budget
for throttling, or a deliberate database migration for writer contention.
A second backend needs shared state and task ownership; local SQLite and workers
cannot simply be duplicated. Distributed tracing helps when requests cross
services. Shorter recovery objectives need validated backup/replication.
