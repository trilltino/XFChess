# triton-bench

Head-to-head RPC suite that proves the Triton integration's value for XFChess by
comparing a Triton endpoint against public devnet. It measures **infrastructure**,
not on-chain CU cost (CUs are deterministic and unaffected by the RPC).

## Probes

| Subcommand | What it proves | Needs |
|------------|----------------|-------|
| `read-load` | Throttling win: read-RPC latency percentiles + HTTP 429 rate under ramping concurrency | nothing |
| `tx-land`   | Landing reliability: `sendTransaction` + confirm timing via memo txs | funded master keypair |
| `stream`    | Push-streaming works (WS pubsub) → settlement can go poll→subscribe | nothing (Windows-friendly) |
| `geyser`    | Same, over Yellowstone **gRPC** | `--features geyser`, Linux/WSL (protobuf toolchain) |
| `all`       | read-load + tx-land + stream (+ geyser if compiled in) | — |

## Setup

Point it at your Triton endpoint (token stays out of source):

```powershell
$env:SOLANA_RPC_URL="https://<host>.devnet.rpcpool.com/<token>"
```

## Run

```powershell
# Throttling comparison (the headline result)
cargo run -p er-cu-benchmark --bin triton-bench -- read-load --requests 200 --levels "1,8,16,32,64,128"

# Transaction landing (needs the master keypair funded with a little devnet SOL)
cargo run -p er-cu-benchmark --bin triton-bench -- tx-land --count 10

# WebSocket push-streaming probe (builds on Windows)
cargo run -p er-cu-benchmark --bin triton-bench -- stream

# Everything
cargo run -p er-cu-benchmark --bin triton-bench -- all
```

### Geyser gRPC (Linux / WSL only)

The Yellowstone client compatible with this workspace's solana 2.2 pin
(`yellowstone-grpc-proto 6.x`) pulls `protobuf-src`, which vendors protobuf via
autotools and **does not build on Windows**. Build it under Linux/WSL:

```bash
cargo run -p er-cu-benchmark --bin triton-bench --features geyser -- geyser
```

`Unauthenticated` / `PermissionDenied` here means Geyser isn't enabled on your
Triton tier — itself a useful answer. On Windows, use `stream` instead (standard
pubsub validates the same poll→subscribe refactor).

## Sample result (devnet, developer tier)

```
── Triton ──
 conc │  ok │ p50 │ 429
   16 │ 128 │  33 │   0
   64 │ 128 │  95 │   0
── public devnet ──
 conc │  ok │ p50 │ 429
   16 │   0 │   0 │ 128   ← fully throttled
   64 │   0 │   0 │ 128
```

Triton absorbs the burst pattern that `settlement_worker` + tournament tasks
generate; public devnet 429s under the same load.

## Notes

- URLs are token-redacted in all output, so logs/screenshots are safe to share.
- `read-load` uses only light, side-effect-free reads (`getSlot`, `getHealth`, …).
- `tx-land` sends SPL-Memo transactions (no accounts, base-fee only).
