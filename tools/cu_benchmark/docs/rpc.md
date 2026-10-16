# Triton RPC probes

These probes measure endpoint performance. They do not estimate game fees or
establish a provider's subscription price. Compare endpoints on the same cluster,
with the same regional routing, workload and commitment.

```powershell
$env:SOLANA_RPC_URL="https://<host>.devnet.rpcpool.com/<token>"
cargo run -p cu_benchmark --bin triton-bench -- read-load --requests 200 --levels "1,8,16,32,64"
cargo run -p cu_benchmark --bin triton-bench -- stream
cargo run -p cu_benchmark --bin triton-bench -- tx-land --count 10
```

`read-load` performs exactly the requested number of lightweight reads per level.
It reports successful latency, throughput, HTTP 429s and errors, including JSON-RPC
errors returned with HTTP 200. These methods do not represent game account reads.
Workers pause together for ten seconds after HTTP 429, as Triton requires;
reported throughput includes that cooldown.
`stream` probes WebSocket delivery. `tx-land` signs paid memo transactions with the
configured master; `all` also sends transactions. Read and stream probes need no key.
Displayed endpoint labels omit credentials, paths and queries; upstream error
messages may still contain sensitive endpoint details.

Optional Yellowstone gRPC requires `--features geyser`, `GEYSER_GRPC_URL` and
`GEYSER_X_TOKEN`. The pinned 6.x protobuf build needs Linux/WSL; it is independent
of the workspace's Solana 3.x transaction client.

[Triton rate limits](https://docs.triton.one/core-features/ratelimits) depend on your
subscription and method mix. [Submission guidance](https://docs.triton.one/chains/solana/cascade/sending-txs)
recommends separate simulation, precise CU limits, competitive priority prices and
client retries. The memo landing probe uses normal preflight and is not a Cascade
optimization benchmark. Simulation and submission are separate billable calls.
Use `cu_benchmark --estimate ... --priority-percentile ...` for fee quotes.
