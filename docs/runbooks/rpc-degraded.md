# RPC degraded

Use RPC latency/error metrics, detailed health and redacted backend logs to
identify timeouts, HTTP 429 or an open primary circuit breaker.

Base reads use `SOLANA_RPC_URL` and `SOLANA_RPC_FALLBACK_URL`; the shared read
adapter has timeouts and circuit-breaker failover. Delegated game submission
uses its MagicBlock routing configuration, not an arbitrary base fallback.

Check the configured provider's status/usage and a same-cluster fallback.
Do not assume a subscription tier or mainnet/devnet from this guide.
Check [RPC probes](../../crates/cu_benchmark/docs/rpc.md) and
[pricing](../../crates/cu_benchmark/docs/pricing.md) for measurement.

For throttling, reduce load and inspect billed method/traffic usage before
changing a plan. If rotating a provider credential, update the protected
environment and restart the service; never post the endpoint token in logs.

A submission timeout is not proof of failure. Reconcile its signature/account
before retrying. Verify healthy detailed checks and normal RPC latency/error
rates. Stuck delegated games follow [ER recovery](magicblock-lifecycle-devnet.md).
