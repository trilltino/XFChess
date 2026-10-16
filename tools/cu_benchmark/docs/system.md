# System workloads

```powershell
cargo run -p cu_benchmark -- --scenario tools/cu_benchmark/scenarios/backend.json
cargo run -p cu_benchmark -- --scenario scenario.json --execute --export run.json
cargo run -p cu_benchmark -- --scenario scenario.json --observations run.json --export repriced.json
cargo test -p backend --test e2e_api cu_benchmark_runs_100_games_through_backend -- --nocapture
```

Without `--execute`, a scenario produces an offline plan and does not contact the
backend. `--observations` recalculates prices and limits from an existing run;
execution inputs must match its fingerprint. Each stage is a separate experiment.

The backend example runs 100 casual games with legal Fool's Mate moves, two
spectators per game, persisted move logs and authenticated result recording.
Set `BENCH_WHITE_WALLET`, `BENCH_BLACK_WALLET`, `BENCH_WHITE_TOKEN` and
`BENCH_BLACK_TOKEN` using existing test accounts. It reuses two players; use
`actors` for a different player mix. The local E2E test also exercises wallet
login and captures the returned bearer tokens into each game's private context.
It writes `cu_benchmark-backend-e2e.json` under the operating system's temporary
directory and sends no chain transactions.

For the configured VPS, [provider pricing](pricing.md) and
`scenarios/hetzner-triton.json` contain researched list rates and their assumptions.
Forward backend/exporter metrics, then run the example from a separate host:

```powershell
ssh -i "$env:USERPROFILE/.ssh/xfchess_vps" -N -L 127.0.0.1:28090:127.0.0.1:8090 -L 127.0.0.1:29100:127.0.0.1:9100 root@178.104.55.19
# In another terminal; creates two test accounts and saved casual game records.
cargo run -p cu_benchmark --example backend_run -- --scenario tools/cu_benchmark/scenarios/hetzner-triton.json --export vps-run.json
```

This run holds 100 games open for roughly a minute. It exercises the HTTP casual
path; long games, anti-cheat analysis and paid settlement need separate variants.
The current VPS lacks the new RPC/process meters. Provider-reported Triton
bandwidth also needs an external usage exporter. Missing coverage stays unknown.

On 2026-10-10, a direct backend run over SSH completed 100 concurrent games,
400 persisted moves and 200 spectator streams without request errors: p95 137 ms,
host CPU peak 32%. Capacity remained inconclusive because RPC/process meters were
missing. The public HTTPS run completed 2/100: SQLite locks, buffered streams and
nginx per-IP result limits caused failures. SQLite and stream config fixes are
implemented locally; these measurements used the existing deployed backend.

Edit stages, move pacing and workflows for the target workload. The short example
deliberately cannot satisfy its 60-second capacity observation requirement;
replace its moves/pacing with a representative sustained workload before drawing
capacity conclusions. Use a separate load-generator host. Host metrics describe
the whole server; backend process metrics describe the backend alone.

Workflow phases are `setup`, `each_move`, `finish` and best-effort `cleanup`.
Requests accept JSON bodies, headers, JSON-pointer captures/expectations and
explicit idempotent retries. HTTP 200 with a failed expectation is a failure.
`${game_id}`, `${game_index}`, `${run_id}`, `${stage}`, `${move_index}` and
`${move.field}` are available; `${env:NAME}` reads an environment variable.
Whole-value substitutions preserve JSON types. Captured tokens, request bodies,
headers and endpoint paths are excluded from reports. `X-Benchmark-Run` connects
requests to backend logs without creating a metric series for every game.

Telemetry selects Prometheus metric names and labels from backend/exporter URLs.
Counters track individual series, including new series and resets. Gauges report
observed minima/maxima; CPU utilization derives busy time from all CPU modes.
Missing required data makes capacity inconclusive. Resource/latency/error limit
violations fail it. Reported capacity is the greatest passing observed concurrency,
not an inferred server maximum. Peaks between scrapes remain unobserved.

The backend exports process memory, CPU cores, process I/O, SQLite pool occupancy,
on-disk session DB/WAL bytes and actual HTTP RPC attempts, including SDK retries.
RPC counters cover clients created through `make_rpc`/`make_async_rpc`; direct
HTTP proxies, WebSocket/gRPC streams and other processes require their own meters.
Configure `node_exporter` URLs via an SSH tunnel and select the real network
interface rather than assuming `eth0`. An empty selector can sum multiple series;
select the billed interface/provider to avoid counting overlapping traffic.

Set pricing `configured: true` only after entering your bills. Fixed costs are
charged once per billing period. Metered costs reference telemetry, a
`quantity_per_unit`, `minor_units_per_unit` and `included_monthly_quantity`.
Counter forecasts add baseline background usage to measured game usage. Byte
gauges forecast retained capacity at period end, rather than transaction traffic;
their cost is a capacity estimate, not an average storage invoice. A rate applies
proportionally to quantity, with final money rounded up to minor units.
Unconfigured pricing and missing usage stay unknown. Combined forecasts require
a passing stage; pricing and chain assumptions remain visible separately.
`prepaid` describes upfront credits and expiry separately from usage expenses.
Costs can declare another `currency`; `exchange_rates_millionths` must supply its
conversion into the report currency, otherwise the combined total stays unknown.

Add `chain: { "fee_profile": "current", "variant": ... }` using the existing
game-cost schema. Its game count is replaced by each stage's count or the monthly
forecast volume. To fetch actual transaction metadata, configure
`transaction_metadata` with `solana_rpc_url_env` and `er_rpc_url_env`; declare each
request's `signature` JSON pointer and `layer` (`solana` or `er`). Enrichment runs
after the measured workload window. Chain forecasts remain modeled, alongside
measured fee/CU coverage; deposits and transfers need separate observations.

For workflows that submit transactions, declare `sends_transactions: true` and
explicitly pass `--allow-transactions`. Staked/tournament workflows must include
their actual provisioning, session, settlement, recovery and background work;
the casual example cannot stand in for those paths. Optional
`capital_delta_lamports` records modeled reservations/releases after successful
steps. Peak reservations exclude undeclared funds, including wagers.
