# Runbooks

One page per alert/incident so on-call can act at 3 a.m. without reverse-engineering the
system. Every SLO-linked alert ([SLO.md](../SLO.md)) must link to a runbook here.

## Index
- [backend-down.md](backend-down.md) — API/health failing
- [settlement-stuck.md](settlement-stuck.md) — wagers not paying out
- [rpc-degraded.md](rpc-degraded.md) — Solana/Triton RPC slow or failing
- [disk-full.md](disk-full.md) — VPS disk pressure

## Template
```
# <Alert name>
**Symptom:** what the alert/user sees
**Severity:** S1 (outage) / S2 (degraded) / S3 (minor)
**Dashboards:** Grafana panel link(s)
## Diagnose
1. ...
## Mitigate
1. ...
## Verify
1. ...
## Root cause / follow-up
- postmortem link
```
