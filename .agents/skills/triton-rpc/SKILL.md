---
name: triton-rpc
description: Measure or debug Triton One Solana RPC, Yellowstone gRPC, transaction delivery, rate limits and game cost estimates in cu_benchmark. Use for RPC reliability, throughput and pricing research.
metadata:
  provenance: docs-derived
---

# Triton One and Yellowstone

Use `crates/cu_benchmark/`, its cost/report models, plus backend RPC
telemetry. Triton One is the Solana provider; this skill does not cover GPU Triton.

- Discover current provider documentation through its index. Verify the endpoint's
  network, region, commitment, authentication and service tier before measuring.
- Keep RPC credentials out of console output and artifacts. Endpoint URLs can
  contain tokens; redact them in reports and errors.
- Separate request latency, submission acceptance, confirmed inclusion and finality.
  Reconcile ambiguous sends by signature/blockhash validity before resubmitting.
- Respect measured/documented rate limits and Retry-After. Bound concurrent load,
  retries and duration; report throttling and failures alongside successful latency.
- For Yellowstone, use the existing feature-gated Rust client/proto versions.
  Check filters, commitment, ping/keepalive, reconnect, duplicate and slot/reorg
  handling. Do not assume the provider offers historical replay without verifying it.
- Compute units measure execution, not currency or VPS CPU. Report transaction
  fees, refundable account deposits, MagicBlock lifecycle charges, RPC subscription
  and server capacity separately; use the installed MagicBlock fee guide for ER costs.
- For game estimates, state game/move counts, concurrency, time controls, payload
  variants, lifecycle mix and measured runtime. Include resource saturation and
  confidence limits; do not extrapolate a single successful simulation to capacity.
- Fetch current Triton and Hetzner prices for pricing tasks. Record date, tier,
  currency, VAT assumptions and evidence; leave quote-only prices unknown.

Use the existing offline report/load fixtures first. Run external load or signed
transactions only within the user's authorized target and budget.

Sources: [Triton docs index](https://docs.triton.one/llms.txt),
[Triton docs](https://docs.triton.one/),
[Yellowstone](https://github.com/rpcpool/yellowstone-grpc),
[Solana RPC](https://solana.com/docs/rpc),
[Hetzner Cloud](https://www.hetzner.com/cloud/).
