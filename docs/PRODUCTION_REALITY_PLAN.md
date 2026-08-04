# XFChess — Production Reality Plan (whole-repo)

Deep evaluation of the production-engineering concepts against **this** codebase, mapped
to idiomatic, modular, documented work across every component. Grounded in the
[Production Reality Checklist](../ops/docs/) (35 domains) and the audits already done:
[VPS](../ops/docs/E2E_REMEDIATION.md), [Frontend](../ops/docs/FRONTEND_REMEDIATION.md),
[Tauri](../tauri/docs/TAURI_REMEDIATION.md).

> **Principle:** adopt what fits a solo-operated, latency-sensitive **P2P chess dapp on a
> single VPS + Solana**, adapt what needs a lighter form, and explicitly **skip** the
> big-tech patterns that would add operational surface without buying reliability at this
> scale. "Skip" always records the trigger that should make us reconsider.

---

## 0 · The stack, honestly

| Component | Path | Tech | Runtime today |
|---|---|---|---|
| Game client | `src/` | Bevy 0.18 ECS, Iroh P2P | Native desktop |
| Backend API | `backend/` | Axum 0.8, SQLx/SQLite, Tokio | systemd on 1 Hetzner VPS |
| Solana program | `programs/xfchess-game/` | Anchor 0.31, Ephemeral Rollups | Solana devnet |
| Web frontend | `xfchessdotcom/` | React 19, Vite | static (nginx) |
| Desktop wrapper | `tauri/` | Tauri 2.10 + Privy | Native; local bridge :7454 |
| Tournament admin | `tauri/tournament-admin/` | React + Tauri shell | Native admin app |
| Shared crates | `crates/` | engine, chess-logic, braid, swiss | in-process |

Single VPS (`178.104.55.19`), nginx reverse proxy, native binaries under systemd,
Prometheus+Grafana (docker-compose), coturn TURN. **No** staging, IaC, object-storage
backups, at-rest DB encryption, or distributed tracing yet.

---

## 1 · Concept verdicts (the "deep evaluate")

Each of your listed concepts, judged for *this* system. **Adopt** = build it. **Adapt** =
build the lightweight idiomatic equivalent. **Skip** = wrong tool at this scale (with the
trigger to revisit).

| Concept | Verdict | Idiomatic form here | Trigger to revisit |
|---|---|---|---|
| **Kubernetes** | ⛔ Skip | systemd units + nginx on 1–2 VPS | >3 nodes or need autoscaling |
| **Docker / Containerisation** | 🟡 Adapt | Already for monitoring; add a backend image for reproducible builds/CI, keep systemd in prod | multi-node or noisy deploys |
| **S3** | ✅ Adopt (compatible) | **Cloudflare R2 / Hetzner Storage Box** for DB backups + PGN/game archives (S3 API via `aws-sdk-s3` or `rusty-s3`) | now — backups have no offsite home |
| **FTP** | ⛔ Skip | Insecure; use `rsync`/`scp` over SSH (deploy already does) | never |
| **Cherry-pick** | ✅ Adopt (process) | Documented hotfix→main→backport flow | now |
| **Staging** | ✅ Adopt | A second cheap VPS or a `staging` systemd slice + devnet; promote same artifact | now — biggest single gap |
| **SQS / Kafka / RabbitMQ** | 🟡 Adapt | Durable **SQLite-backed job queue** (`tasks/`) w/ idempotency + DLQ table; not a broker | cross-service fan-out or >1 worker host |
| **Serverless / Lambda** | ⛔ Skip (mostly) | Game server is stateful (P2P/ER); the **Solana program + Blinks** are the "serverless" compute | spiky stateless edge work |
| **Cloud** | 🟡 Adapt | Stay VPS-first; use managed pieces only where they de-risk (RPC, object storage, email) | cost/ops of self-hosting exceeds managed |
| **CI/CD** | ✅ Adopt (harden) | GH Actions: test+scan gates → staging → prod, same artifact, traceable | now |
| **Web sockets** | ✅ Have (harden) | `/ws/auth` + braid live subscriptions; add heartbeats/backpressure | now |
| **Long/short polling** | ✅ Have | `ws_subscriber` (default) vs `polling` feature flag; document when each is used | — |
| **Encryption** | 🟡 Adopt gaps | TLS in transit ✅; identity fields encrypted ✅; add **SQLite at-rest** (SQLCipher or FS-level) + backup encryption | pre-mainnet / PII scale |
| **Firewall** | ✅ Adopt | UFW + fix **Docker-bypasses-UFW** (bind monitoring to localhost) | now (from VPS audit) |
| **TensorFlow** | ⛔ Skip | Chess AI is `nimzovich_engine` (classical search). If ever ML eval → **ONNX Runtime**, not TF | never (use ONNX if ML) |
| **Database** | ✅ Keep (SQLite) | Embedded SQLite is right; document WAL, backups, and the Postgres migration path | write contention at scale |
| **Embedded database** | ✅ Have | SQLite (`sessions.db`, `vault.db`) — WAL, `busy_timeout`, single-writer discipline | — |
| **DynamoDB** | ⛔ Skip | No AWS coupling; SQLite→Postgres if relational scale needed | — |
| **Sharding / Partitioning** | 🟡 Adapt | Not needed at row counts today; **time-partition** `games`/`moves`, archive cold rows to R2 | tables >10M rows or slow scans |
| **Caching** | ✅ Adopt | In-proc TTL cache (`moka`) for leaderboard/ratings/RPC reads; document TTLs + stampede guard | now — RPC reads repeat |
| **Rate limiting** | ✅ Have (extend) | nginx zones (fixed in VPS audit) + app-layer per-wallet/IP; per-endpoint | now |
| **Throughput / QPS** | ✅ Measure | Prometheus RED metrics + `er-cu-benchmark`; publish p50/p95/p99 | now |
| **RPC** | ✅ Harden | Solana RPC (Helius/Triton) w/ timeouts, failover, response cache, circuit breaker | now |
| **Error logging** | ✅ Harden | `tracing` + crash reporting (`core/`); add structured JSON + correlation IDs + Sentry-like sink | now |
| **Load balancer** | 🟡 Defer | nginx is the single-node LB; document HA (2 nodes + floating IP) design | second app node |
| **Proxy** | ✅ Have | nginx reverse proxy; trust `X-Forwarded-*` safely (real-IP allowlist) | now |
| **Optimisation** | ✅ Ongoing | Tracy profiling (`just profile`), release opt-levels, query/index review | continuous |
| **Availability** | ✅ Adopt | Define SLIs/SLOs, health/readiness, degraded modes, restore drills | now |
| **Git/GitHub** | ✅ Adopt | Branch protection, signed hotfixes, PR gates, commit→deploy traceability | now |
| **Deployments** | ✅ Harden | Zero-downtime restart, one-command rollback (exists), expand-contract migrations | now |
| **PyCharm** | ⛔ N/A | Rust/TS repo — RustRover/VS Code + rust-analyzer; standardize `.editorconfig`/settings | — |

**Net:** the genuinely high-value adopts are **Staging, offsite encrypted backups (S3-compatible),
CI/CD gates, caching, RPC resilience, observability/tracing, and the SLO/runbook layer.**
Kubernetes, Kafka, DynamoDB, Lambda, TensorFlow are the wrong tools here — documented as skips.

---

## 2 · Cross-cutting workstreams (modular, with owners in code)

Each workstream lands as a **self-contained module + a doc**, so it's idiomatic and reviewable.

### WS-A · Reliability & Async (`backend/src/tasks/`, new `queue/`)
- **Durable job queue in SQLite** for work with **no durable backing of its own** (email
  sends; future webhooks/notifications): table `jobs(id, kind, payload, run_at, attempts,
  status, dedupe_key)`, a poller, **bounded retries w/ backoff+jitter**, and a **DLQ**
  (`status='dead'`) with a Grafana panel. Idempotent enqueue via `dedupe_key`.
- **Design decision (revised on implementation):** `settlement_worker` and prize
  distribution stay **scan-based** — they re-derive work from on-chain state every tick,
  so the chain *is* their durable queue; mirroring into SQLite would create a second
  source of truth. Anti-cheat likewise already has a re-ingest sweep from the games table.
- **Timeouts on every external call** (RPC, email, RPC-to-ER); **circuit breaker** wrapper (`crates/` util or `backend/src/signing/solana/rpc.rs`).
- Doc: `docs/RELIABILITY.md` (failure modes table, degraded modes, RPO/RTO).

### WS-B · Data & Backups (`backend/src/db/`, new `ops/backup/`)
- Document source-of-truth per entity (on-chain vs SQLite vs cache) and consistency model.
- **Backup job**: nightly `VACUUM INTO` snapshot → **encrypt (age/gpg)** → push to **R2** (S3 API), retention + immutability (object-lock). **Test restore** and record the date in `docs/DR.md`.
- Migrations: enforce **expand-contract**, add a `sqlx migrate` dry-run + rollback test in CI.
- At-rest: evaluate **SQLCipher** for `vault.db`/`identity`; interim = LUKS/FS encryption on the VPS volume.
- Partitioning/archival plan for `games`/`moves` (time buckets → cold archive to R2).

### WS-C · Caching & Performance (`backend/src/cache/` new)
- `moka` async TTL cache module: leaderboard, player profiles, SOL/USD price, hot RPC reads.
- Documented TTLs + **single-flight** (stampede protection) + tenant/user-scoped keys.
- Load test with `er-cu-benchmark`; publish p95/p99 and capacity at 10× in `docs/CAPACITY.md`.

### WS-D · RPC & External Resilience (`backend/src/signing/solana/rpc.rs`)
- Wrap all Solana RPC in a client with: connect/read timeouts, **primary→fallback endpoints** (Helius→public→Triton), retry policy, **circuit breaker**, and a **read cache**. Metrics per endpoint.
- Same pattern for email (Resend) and ER RPC. Doc provider fallbacks in `docs/VENDORS.md`.

### WS-E · Observability (`backend/src/telemetry/`, all services)
- **Structured JSON logs** (`tracing-subscriber` json) with **correlation/request IDs** propagated client→backend→chain; **redact secrets** (deny-list layer).
- Expand RED metrics (rate/errors/duration) per route + worker; alerts mapped to SLOs (`ops/monitoring/rules/`), **every alert gets a runbook** in `docs/runbooks/`.
- Add lightweight **distributed tracing** (OpenTelemetry OTLP → Grafana Tempo, optional) or at minimum correlation-ID logging end-to-end.
- Error sink (self-hosted GlitchTip / Sentry) for grouped, actionable errors from game + web + backend.

### WS-F · Security & Supply Chain (repo-wide)
- Land the three audits' fixes (secrets rotation, CORS allowlist, nginx routing/rate-limit/headers, Tauri capability split, bridge token).
- CI: `cargo audit` + `cargo deny` (licenses/bans) + `npm audit` gates; **SBOM** (`cargo cyclonedx`) + artifact signing (relates to distribution memory).
- Threat model doc `docs/THREAT_MODEL.md` (attacker/abuse/fraud paths: move forgery, session-key abuse, wager settlement fraud, Sybil, bridge token theft).
- OWASP Top-10 pass on backend + web; upload/PGN import validation.

### WS-G · Delivery: Envs, CI/CD, Git (repo + `.github/`, `ops/`)
- **Staging environment** (second VPS or namespaced systemd slice; devnet). Promote the **same artifact** staging→prod.
- Branch protection: block direct push to `main`, require PR + review; **dedicated reviewers** for `migrations/`, `programs/`, `ops/`. Signed hotfix + **cherry-pick/backport** process → `docs/GIT_WORKFLOW.md`.
- CI/CD: build once → test+scan gates → deploy staging → smoke → deploy prod; **DORA metrics**; rollback one-command (exists) + tested; deployment traceable to commit (embed git SHA in `/health`).

### WS-H · Availability & Ops (`docs/`, `ops/`)
- Define **SLIs/SLOs** per flow (auth, move record, settlement, tournament ops, web reads) with p50/p95/p99 + error budget → `docs/SLO.md`.
- Health/readiness/liveness that mean something (DB reachable, RPC reachable, ER reachable); degraded-mode banners in client/web.
- **Incident response**: severity levels, runbooks, blameless postmortem template → `docs/runbooks/`, `docs/INCIDENT_RESPONSE.md`.
- **DR**: RPO/RTO, restore drill cadence, regional-loss runbook → `docs/DR.md`.

---

## 3 · Per-component rollup

### VPS / Cloud infra
Adopt: UFW + Docker-bypass fix, TLS auto-renew verify, real-IP/forwarded-header hygiene, offsite encrypted backups (R2), staging host, minimal-ports, monitoring bound to localhost. Skip: k8s, multi-region (until traction). Docs: `E2E_REMEDIATION.md` (exists) + `docs/DR.md`, `docs/SLO.md`.

### Backend (Axum)
Adopt: durable queue+DLQ, caching layer, RPC resilience, structured logs+correlation IDs, per-wallet/IP app rate limits, graceful SIGTERM drain, env validation at startup (fail fast). Keep: SQLite (WAL). Modules: `tasks/queue/`, `cache/`, `signing/solana/rpc.rs`, `telemetry/`.

### Game client (Bevy) & Networking (Iroh/braid)
Adopt: reconnect/backoff on relay, heartbeats + backpressure on WS/QUIC, deterministic resync after restart, crash reporting sink, timeouts on all backend calls, client-version gate (forced-upgrade) + old-client↔new-API compat. Document sync-vs-async boundaries and P2P trust model (`docs/TRUST_MODEL.md` exists — extend). Long/short polling already flagged.

### Contracts (Anchor program)
Adopt: invariant tests (perft + on-chain differential exist — extend), expand-contract account migrations, upgrade authority governance + rollback plan, mainnet deploy checklist, dispute/settlement abuse tests. Keep on-chain validation as source of truth for moves. Doc: `docs/CONTRACT_OPS.md` (upgrade, authority rotation → ties to `SECRETS_ROTATION.md`).

### Tournament admin (Tauri) & Admin/Support
Adopt: restrict + **audit** all admin actions, break-glass process, impersonation logged/approved, admin app hardened like prod (Tauri audit fixes: shell scoped to this window only), no one-click prod-deploy button shipped. Doc: `docs/ADMIN.md`.

### Web frontend + Desktop wallet
Adopt: JWT→httpOnly cookie (or short-TTL + refresh), Helius key behind backend proxy, CSP, Privy integration hardening, forced-upgrade, secure token storage (Tauri bridge token). Docs: `FRONTEND_REMEDIATION.md`, `TAURI_REMEDIATION.md` (exist).

---

## 4 · Phased roadmap

**P0 — before any real-money / mainnet (safety & data):**
1. Land all three audit remediations (secrets, CORS, nginx routes/rate-limit/headers, Tauri caps + bridge token). WS-F.
2. Offsite **encrypted backups + tested restore** (R2) + `docs/DR.md`. WS-B.
3. **Staging** env + same-artifact promotion + branch protection. WS-G.
4. Env validation at startup, timeouts + circuit breaker on RPC/email, graceful shutdown. WS-A/D.
5. SLOs + health/readiness + alert→runbook baseline. WS-E/H.

**P1 — scale & robustness:**
6. Durable SQLite job queue + DLQ (migrate settlement/prizes). WS-A.
7. Caching layer + load test + capacity doc. WS-C.
8. Structured logs + correlation IDs + error sink. WS-E.
9. CI gates: `cargo audit`/`deny`, `npm audit`, SBOM, artifact signing. WS-F/G.
10. Threat model + OWASP pass + abuse/fraud controls. WS-F.

**P2 — maturity / growth:**
11. Distributed tracing (OTel→Tempo). WS-E.
12. Time-partition + cold archival of games/moves. WS-B.
13. HA design (2 app nodes + floating IP) — *design only until traffic warrants*. WS-H.
14. At-rest DB encryption (SQLCipher). WS-B.

---

## 5 · Production gate (checklist answers, current state)

| # | Question | Current answer / gap |
|---|----------|----------------------|
| 1 | What can fail? | RPC/ER outage, VPS down, SQLite corruption, relay partition, settlement stuck. *Enumerate in `docs/RELIABILITY.md`.* |
| 2 | How will we know? | Prometheus/Grafana + `/health`. **Gap:** SLO alerts + error sink. |
| 3 | Who is alerted? | Alertmanager configured; **owner/rotation undefined.** |
| 4 | What do they do? | **Gap:** runbooks. |
| 5 | How do we recover? | Restart via systemd; settlement worker self-heals. **Gap:** DR runbook. |
| 6 | How do we roll back? | `ops/scripts/rollback.ps1` exists; **time-to-rollback untracked.** |
| 7 | Prevent data loss? | On-chain is durable; SQLite **has no offsite backup** → P0. |
| 8 | Prevent unauthorized access? | JWT + session keys + CACF; **CORS/token gaps** being fixed. |
| 9 | Protect secrets? | `.env` + rotation doc; **history leak** → rotate (P0). |
| 10 | Duplicate requests? | On-chain idempotent; **backend jobs need dedupe** (WS-A). |
| 11 | Retries? | Ad-hoc; **standardize backoff+jitter** (WS-A/D). |
| 12 | Slow dependencies? | **Add timeouts + circuit breakers** (WS-D). |
| 13 | Bad deployments? | Manual; **add staging gate + smoke** (WS-G). |
| 14 | Schema changes? | SQLx migrations; **enforce expand-contract + test** (WS-B). |
| 15 | Restore from backup? | **Not tested** → P0 (WS-B). |
| 16 | Support users? | Tournament-admin; **audit + break-glass** (WS component). |
| 17 | Control cost? | Low (1 VPS); **budget alerts + RPC/email caps** (WS-D). |
| 18 | Prove what happened? | Partial logs; **audit logs + correlation IDs** (WS-E). |
| 19 | Who owns it post-launch? | Solo — **bus-factor risk**; docs are the mitigation. |

---

## 5b · Execution status (living)

| Phase | Status | Landed |
|---|---|---|
| **P0** | ✅ done | WS-B backups ([backup-db.sh](../ops/backup/backup-db.sh), [DR.md](DR.md)); WS-D RPC resilience + Triton primary ([rpc.rs](../backend/src/signing/solana/rpc.rs)); WS-A validate+graceful shutdown; WS-H [SLO.md](SLO.md) + `/readyz` + git SHA + [runbooks/](runbooks/); WS-F nginx + CI scans; WS-G [GIT_WORKFLOW.md](GIT_WORKFLOW.md) + [ENVIRONMENTS.md](ENVIRONMENTS.md) |
| **P1** | ✅ done | WS-A durable [job queue](../backend/src/tasks/queue.rs) + DLQ (email); WS-C [CAPACITY.md](CAPACITY.md) (Helius key leak fixed, elo RPC timeout); WS-E JSON logs + `x-request-id`; WS-F [THREAT_MODEL.md](THREAT_MODEL.md) |
| **P2** | 📐 designed | [SCALING.md](SCALING.md) — OTel, partition/archival, HA, at-rest, Litestream (each with a build trigger) |

Verification: backend `cargo test` green (129+), `cargo check` clean, web `npm run build` green.

## 6 · How this stays modular & documented

- Each workstream = **one module** (`backend/src/{queue,cache}/`, `signing/solana/rpc.rs`, `telemetry/`) with a unit-tested public surface and **one doc** under `docs/`.
- Every new external dependency passes the concept verdict table (§1) — no adoption without a "why here, not the big-tech default."
- Docs live beside code and are updated in the same PR (checklist §34). This file is the index; link children as they land.

> **Next:** confirm the P0 ordering and I'll execute WS-F remainder + WS-B (backups) first, since those are the "real-money safety" blockers. Nothing here is committed yet.
