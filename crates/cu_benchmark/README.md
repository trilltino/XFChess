# cu_benchmark

Estimate game costs, measure backend capacity and benchmark RPC endpoints.
Commands below run from the repository root.

| Binary | Purpose |
| --- | --- |
| `cu_benchmark` | Offline cost models, backend workloads and opt-in live chain measurements |
| `triton-bench` | RPC reads, transaction landing and stream probes |
| `inspect` | Account ownership, allocated bytes, balance and rent |

```powershell
cargo run -p cu_benchmark -- --estimate crates/cu_benchmark/scenarios/games.json
cargo run -p cu_benchmark -- --scenario crates/cu_benchmark/scenarios/backend.json
cargo run -p cu_benchmark -- --scenario scenario.json --execute --export run.json
cargo run -p cu_benchmark -- --scenario scenario.json --observations run.json --export repriced.json
cargo run -p cu_benchmark --bin inspect -- <account-public-key>
```

Estimates and scenario plans run offline. `--execute` runs the configured backend
workflow; transaction workflows also require `--allow-transactions`.
`--observations` reprices a saved run while enforcing the same workload.
Missing measurements and unconfigured prices remain unknown.

- [System workloads and capacity](docs/system.md)
- [Game cost models and live measurements](docs/costs.md)
- [RPC probes](docs/rpc.md)
- [Hetzner and Triton pricing](docs/pricing.md)

## Layout and checks

| Folder | Responsibility |
| --- | --- |
| `src/cost/` | Fee models, quotes, transaction measurements and cost reports |
| `src/chain/` | Accounts, wallets, instruction builders, game flows and recovery |
| `src/system/` | Workload scenarios, HTTP execution, telemetry, capacity and infrastructure pricing |
| `src/rpc/` | Read, transaction landing and stream probes |
| `src/cli/`, `src/bin/` | Command parsing and entry points |
| `scenarios/` | Editable JSON workloads |
| `examples/` | Runnable Rust examples |
| `docs/` | Provider pricing research and RPC guidance |
| `tests/` | Workload and CLI integration tests |

The runner separates game lifecycles, HTTP requests, subscriptions and template
rendering. Tournament instruction builders separate setup, sessions, matches
and settlement. Commands and report formats remain unchanged.

```powershell
cargo test -p cu_benchmark --all-targets
cargo clippy -p cu_benchmark --all-targets
```
