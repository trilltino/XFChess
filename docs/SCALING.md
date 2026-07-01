# Scaling & Maturity (P2) — XFChess

Design-ahead items that don't pay off yet at single-VPS + devnet scale, documented so
they're ready to switch on when a trigger fires. Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) P2. Each has an explicit **trigger**
— don't build before it.

## 1 · Distributed tracing (OpenTelemetry)
**Now:** structured JSON logs + `x-request-id` correlation across a request (WS-E). Enough
to trace within the single backend.
**Trigger:** a second service in the hot path (e.g. split signing-server / relay / a worker
host) where you need to follow one request across process boundaries.
**Plan:** add `tracing-opentelemetry` + `opentelemetry-otlp`, export spans to **Grafana
Tempo** (already have the Grafana stack). Propagate `traceparent` alongside the existing
`x-request-id`. Keep JSON logs; add trace/span IDs to them. ~1 module (`telemetry/otel.rs`),
gated on `OTEL_EXPORTER_OTLP_ENDPOINT` so it's a no-op when unset.

## 2 · DB partitioning & cold archival
**Now:** SQLite WAL, one file. Games/moves grow unbounded but row counts are tiny.
**Trigger:** `games`/`moves` tables slow queries or the DB file gets large (>~a few GB), or
`/health/detailed` disk check trends up.
**Plan:**
- **Time-bucket** archival: monthly, copy settled games older than N months into an archive
  table or a separate `archive-YYYYMM.db`, then delete from the live table (a job-queue
  `archive.sweep` kind fits WS-A).
- Push cold archives to **R2** (reuse the WS-B backup path) and drop them locally.
- The existing `tasks/archiver.rs` is the seam — extend it rather than adding a system.
- Postgres migration path (if write contention appears first): SQLx already abstracts the
  driver; the main work is the connection string + a handful of SQLite-isms.

## 3 · High availability (multi-node)
**Now:** single Hetzner VPS; nginx is the single-node reverse proxy/LB; SQLite is local.
This is a deliberate SPOF trade for simplicity — a VPS reboot is minutes of downtime,
acceptable pre-scale (see [SLO.md](SLO.md)).
**Trigger:** SLO breaches from single-node outages, or sustained load beyond one box.
**Plan (design):**
- **App tier:** make the backend fully stateless w.r.t. local disk (move SQLite → managed
  Postgres, or a replicated store) so 2+ nodes can run behind a load balancer.
- **LB:** Hetzner Load Balancer or a floating IP + keepalived across 2 nodes.
- **State:** Postgres (primary + replica) or Litestream-replicated SQLite for a cheaper
  interim (streams the WAL to R2 for point-in-time recovery — pairs well with WS-B).
- **Sessions:** already disposable (users re-login), so no sticky-session requirement.
- Keep the P2P relay concerns separate — Iroh sessions are per-game and can shard by node.

**Cheapest first step:** add **Litestream** on the current box now (continuous SQLite
replication to R2) — it upgrades RPO from 24h (daily backup) to seconds without going
multi-node. Consider promoting this out of P2 if RPO matters before HA does.

## 4 · Encryption at rest
**Now:** identity/KYC **fields** are app-encrypted (AES-GCM); TLS in transit; backups
age-encrypted. The SQLite files themselves are plaintext on disk.
**Trigger:** before mainnet / handling real KYC at volume, or any compliance requirement.
**Plan:**
- **Interim (no code):** enable **full-disk / volume encryption** (LUKS) on the VPS data
  volume — protects `/opt/xfchess/data` at rest with zero app changes.
- **Stronger:** **SQLCipher** for `vault.db` (page-level encryption, key from env/KMS).
  Requires swapping the SQLite driver build; validate SQLx compatibility first.
- Keep the encryption key out of the DB host's backups (separate from `.env`).

## Trigger summary
| Item | Build when |
|---|---|
| OTel tracing | 2nd hot-path service |
| Partition/archival | tables slow or DB file large |
| HA / multi-node | single-node outages breach SLO |
| Litestream (RPO↑) | RPO < 24h needed (can do now, cheap) |
| At-rest (LUKS/SQLCipher) | pre-mainnet / compliance |
