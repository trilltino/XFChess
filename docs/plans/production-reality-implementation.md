# XFChess Production Reality Implementation Plan

Status: **repo-aware implementation plan**. Created from the external
"Production Reality" checklist after reviewing the first-party XFChess codebase:
Rust/Bevy game client, Anchor Solana program, Axum backend, React web app, Tauri
desktop wrapper, deployment scripts, monitoring config, CI, legal docs, and
existing planning docs. Vendored/reference trees and build output were treated as
non-shipping material.

Related docs:

- [ops/docs/E2E_REMEDIATION.md](../../ops/docs/E2E_REMEDIATION.md) -
  hosted deployment remediation.
- [ops/docs/FRONTEND_REMEDIATION.md](../../ops/docs/FRONTEND_REMEDIATION.md)
  - frontend audit/remediation.
- [ops/docs/ROLLBACK_GUIDE.md](../../ops/docs/ROLLBACK_GUIDE.md) -
  rollback mechanics.
- [ops/SECRETS_ROTATION.md](../../ops/SECRETS_ROTATION.md) - secret
  rotation.

## Executive Readiness View

XFChess is past prototype scale: it has real money-adjacent flows, KYC/CACF
gating, tournament state, on-chain escrow, background settlement, anti-cheat,
Privy/wallet surfaces, deployment scripts, Nginx rate limits, Prometheus/Grafana,
and CI. Production readiness should therefore be treated as a launch gate, not a
nice-to-have cleanup.

Current strengths:

- Clear first-party architecture exists across game, backend, program, web,
  desktop, and shared crates.
- Backend has auth hardening tests, admin API key middleware, route E2E tests,
  CORS env support, metrics endpoint, health endpoint, and background workers.
- On-chain chess legality has differential tests through `chess-logic-on-chain`
  and `nimzovich_engine`.
- Deploy assets exist for Hetzner/systemd/Nginx/rollback/monitoring/secrets.
- Legal and compliance thinking is present, including jurisdiction docs and
  CACF gating.

Main risks before production:

- Secrets and environment hygiene are not fully settled. Existing remediation
  docs already call out committed/duplicated production env files and required
  rotation.
- Database migrations are idempotent scripts run at startup, but there is no
  explicit migration ledger, rollback contract, restore drill, or zero-downtime
  migration gate.
- Monitoring exists, but SLOs, alert ownership, dashboards-as-incident-tools,
  trace/correlation, and incident runbooks are incomplete.
- Release flow deploys directly to the VPS and restarts services, but production
  gates, canaries, staged promotion, artifact provenance, and post-deploy smoke
  checks need to be formalized.
- Web/desktop supply-chain and client-side security need gates. `xfchessdotcom`
  depends on fast-moving wallet/Privy/Solana packages and current remediation
  docs flag frontend audit work.
- Money, KYC, tournament, and escrow paths need explicit owner sign-off,
  rollback decisions, legal sign-off, and abuse controls before mainnet/wagering.

## Production Definition

XFChess is production-ready only when these are true:

1. A new operator can deploy, verify, roll back, rotate secrets, and restore data
   using checked-in docs.
2. A user-facing outage, exploit attempt, bad deploy, database corruption, stuck
   tournament, failed settlement, vendor outage, or Solana/MagicBlock outage has
   an owner, alert, runbook, and tested recovery path.
3. The system can prove what happened for money-affecting actions: game creation,
   move recording, finalization, disputes, KYC decisions, prize distribution,
   admin actions, and secret rotation.
4. All wager/prize flows are protected by KYC/CACF, auth, authorization,
   anti-cheat gates, rate limits, replay/idempotency controls, and audited
   contracts.
5. CI blocks unsafe changes across Rust, Solana program, web, desktop, deploy,
   and migration surfaces.

## Phase 0 - Freeze The Shipping Surface

Goal: decide what is actually in the first production launch.

Tasks:

- Define launch mode: `devnet-only`, `mainnet-no-wagers`, `mainnet-prize`,
  or `mainnet-wagering`.
- Create a production feature matrix for Bevy, web, backend, Tauri, and program.
- Mark non-launch paths as disabled, hidden, or dev-only.
- Decide supported regions, age/KYC policy, and legal entity text. Replace
  placeholder web copy such as `[YOUR LEGAL ENTITY NAME]` before public launch.
- Decide support model: support mailbox, security mailbox, KYC escalation,
  dispute escalation, and tournament operator escalation.

Acceptance gate:

- `docs/LAUNCH_SCOPE.md` exists with launch mode, features, disabled paths,
  legal/support owners, and "no launch" approvers.

Primary files:

- `xfchessdotcom/src/pages/Compliance.tsx`
- `xfchessdotcom/src/pages/Legal.tsx`
- `docs/XFChess_Jurisdictional_Deep_Dive.txt`
- `legal/*`
- `docs/IMPLEMENTATION_PLAN.md`

## Phase 1 - Secrets, Config, And Environment Hygiene

Goal: remove ambiguity around production config and prevent secret leakage.

Tasks:

- Finish the remediation in `ops/docs/E2E_REMEDIATION.md` for production env
  files and secret rotation.
- Ensure `.env`, `ops/.env.production`, `ops/backend/.env.production`, and
  `xfchessdotcom/.env.production` are either untracked local templates or removed
  from git history and rotated if previously committed.
- Add runtime config validation on backend startup: required envs, key lengths,
  allowed origins, Solana RPC URLs, admin API key, relay secret, database paths,
  and mainnet/devnet consistency.
- Fail closed in production if `ALLOWED_ORIGINS` is missing.
- Document one canonical env file per deployment target and one owner for each
  secret.
- Add a CI secret scan and dependency audit gate.

Acceptance gate:

- `ops/SECRETS_ROTATION.md` has been executed for all compromised or uncertain
  secrets.
- Backend exits before binding if production config is incomplete or unsafe.
- CI blocks accidental secret commits.

Primary files:

- `backend/src/signing/config.rs`
- `backend/src/signing_server.rs`
- `backend/src/infrastructure/router.rs`
- `ops/backend/.env.example`
- `ops/SECRETS_ROTATION.md`
- `.github/workflows/ci.yml`

## Phase 2 - Data Safety, Migrations, Backup, Restore

Goal: make SQLite acceptable for the chosen launch scale, or explicitly migrate
off it before launch.

Tasks:

- Choose production datastore posture: keep SQLite with strict operational
  limits, or migrate to Postgres for user/tournament/money-adjacent state.
- Add a schema migration ledger instead of relying only on idempotent startup
  scripts.
- Treat migrations as append-only and tested. Add CI that applies every
  migration to an empty DB and to a fixture DB.
- Document which data lives in `session_pool` vs `vault_pool`, retention rules,
  encryption expectations, and restore priority.
- Add backup jobs for session DB, vault DB, Prometheus/Grafana state if retained,
  and server config.
- Add restore drills: full restore, vault-only restore, single customer/tournament
  restore decision, and rollback after incompatible migration.
- Verify deployment backup/rollback scripts against the actual deployed paths.

Acceptance gate:

- `docs/RUNBOOK_BACKUP_RESTORE.md` exists and includes a dated successful restore
  drill.
- A migration PR cannot merge without migration tests and rollback notes.

Primary files:

- `backend/src/infrastructure/database.rs`
- `backend/migrations/*`
- `ops/scripts/deploy.ps1`
- `ops/scripts/rollback.ps1`
- `ops/docs/ROLLBACK_GUIDE.md`

## Phase 3 - Auth, Authorization, Admin, And Auditability

Goal: every privileged or money-affecting action must be authenticated,
authorized, idempotent where needed, and auditable.

Tasks:

- Inventory every route from `backend/src/signing/routes/*` and mark it public,
  user-auth, relay-auth, admin-auth, or internal.
- Replace any "admin token in body" pattern with headers/middleware only.
- Ensure object-level authorization for user, wallet, tournament, dispute,
  profile, KYC, and game-history reads/writes.
- Add idempotency keys for payment/wager/prize/tournament creation and any
  retryable mutating endpoint.
- Add tamper-resistant audit records for admin actions, KYC decisions, dispute
  resolution, secret rotation, settlement, prize distribution, and CACF decisions.
- Create break-glass access policy and require audit logging for it.

Acceptance gate:

- `docs/ROUTE_AUTHORIZATION_MATRIX.md` exists and matches the router.
- Backend E2E tests cover unauthenticated, wrong-user, wrong-admin, duplicate,
  and replay attempts for critical routes.

Primary files:

- `backend/src/infrastructure/router.rs`
- `backend/src/infrastructure/auth_middleware.rs`
- `backend/src/signing/routes/*`
- `backend/tests/e2e_api.rs`
- `backend/migrations/*audit*`

## Phase 4 - SLOs, Observability, And Incident Response

Goal: know when users are affected and what the operator should do.

Tasks:

- Define SLIs/SLOs for: backend availability, auth, session signing, move
  recording, tournament join/create, settlement worker, prize distribution,
  KYC submission, frontend availability, and Solana/MagicBlock dependency health.
- Add request IDs/correlation IDs across frontend, game client, backend logs,
  Solana tx signatures, and worker jobs.
- Convert metrics to useful counters/histograms for latency, error rate, queue
  backlog, worker lag, failed settlements, stuck tournaments, RPC failures, and
  fee-payer balance.
- Add alert rules that map to symptoms and runbooks, not just raw causes.
- Create runbooks for backend down, high 5xx, high tx failure, RPC outage,
  MagicBlock outage, stuck settlement, stuck tournament, KYC outage, suspicious
  admin action, and suspected secret leak.
- Decide who is on-call and how users are notified.

Acceptance gate:

- `docs/SLO.md` and `docs/INCIDENT_RUNBOOKS.md` exist.
- Prometheus alerts have an owner and runbook link.
- A tabletop incident has been run and recorded.

Primary files:

- `backend/src/telemetry/*`
- `backend/src/tasks/*`
- `ops/monitoring/prometheus.yml`
- `ops/monitoring/rules/*`
- `ops/monitoring/grafana/dashboards/xfchess.json`
- `ops/README.md`

## Phase 5 - CI/CD, Release, Rollback, And Supply Chain

Goal: deployments are repeatable, traceable, and reversible.

Tasks:

- Make CI required for protected branches. Remove `continue-on-error` from
  security-relevant gates once warnings are cleaned.
- Add web lint/build/audit jobs and Tauri build smoke jobs.
- Add Anchor program build/test gates, including local-validator tests where
  available and nightly longer tests.
- Generate SBOMs for Rust, Node, Tauri bundle, Docker images, and release
  artifacts.
- Sign release artifacts and record provenance.
- Promote the same immutable artifact from staging to production rather than
  rebuilding on deploy.
- Add post-deploy smoke checks for `/health`, `/metrics`, frontend route load,
  auth route, tournament read route, and a non-mutating Solana/RPC check.
- Decide rollback vs roll-forward policy for migrations and on-chain program
  upgrades.

Acceptance gate:

- `docs/RELEASE_PROCESS.md` exists and every production deploy records commit,
  artifact, operator, tests, smoke results, and rollback plan.

Primary files:

- `.github/workflows/ci.yml`
- `.github/workflows/deploy.yml`
- `.github/workflows/release.yml`
- `ops/scripts/deploy.ps1`
- `ops/scripts/rollback.ps1`
- `backend/Dockerfile`
- `xfchessdotcom/package.json`
- `Cargo.lock`

## Phase 6 - Security, Abuse, And Compliance Hardening

Goal: make the obvious attacks boring and the serious attacks detectable.

Tasks:

- Produce a threat model for: wallet/session delegation, relay secret, admin
  routes, KYC/vault data, tournament escrow, prize distribution, anti-cheat,
  web wallet auth, Tauri IPC, and Solana program upgrade authority.
- Add abuse controls for signup, waitlist/mail, auth, KYC submit, matchmaking,
  tournament join/create, move record, chat, profile lookup, and public history.
- Confirm Nginx rate limits cover the actual public route paths and that backend
  application-level limits exist for routes not always behind Nginx.
- Add content security policy for the web frontend, not only Tauri.
- Add dependency, license, vulnerability, and outdated-package tracking.
- Commission or perform a smart-contract security audit before mainnet wagering
  or prize escrow.
- Lock down program upgrade authority, KYC authority, fee vault authority, link
  authority, and release signing credentials.
- Add privacy/data-retention docs for KYC, audit logs, gameplay, chat, emails,
  and analytics.

Acceptance gate:

- `docs/THREAT_MODEL.md`, `docs/ABUSE_CONTROLS.md`, and `docs/DATA_RETENTION.md`
  exist and critical controls are tested.
- No mainnet wager/prize feature is enabled without program audit sign-off.

Primary files:

- `SECURITY.md`
- `ops/nginx/nginx.conf`
- `ops/nginx/xfchess_rate_limit.conf`
- `tauri/tauri.conf.json`
- `tauri/capabilities/*`
- `programs/xfchess-game/src/*`
- `xfchessdotcom/src/lib/api/*`

## Phase 7 - Product Operations And Support

Goal: launch with human operations that match the product promises.

Tasks:

- Define support SLAs, refund/dispute policy, tournament cancellation policy,
  anti-cheat appeal policy, and KYC rejection policy.
- Build or document admin workflows for stuck tournaments, stuck settlements,
  disputes, KYC review, player suspension, prize distribution review, and fee
  payer top-up.
- Ensure admin tooling is secured and audited like customer-facing code.
- Add public status page decision and incident communication templates.
- Decide whether telemetry/analytics are consent-aware and privacy-reviewed.
- Create a pre-launch operational readiness review checklist.

Acceptance gate:

- `docs/OPERATIONAL_READINESS_REVIEW.md` is completed before public launch.

Primary files:

- `backend/src/bin/tournament_admin.rs`
- `backend/src/bin/vps_admin.rs`
- `tauri/tournament-admin/*`
- `backend/src/signing/routes/admin.rs`
- `backend/src/signing/routes/dispute.rs`
- `xfchessdotcom/src/pages/Compliance.tsx`

## Critical Path

Do these in order:

1. Phase 0: launch scope and disabled surfaces.
2. Phase 1: secrets/config hardening and rotation.
3. Phase 2: backups, restore drill, migration ledger.
4. Phase 3: route authorization matrix and audit logs.
5. Phase 4: SLOs, alerts, runbooks.
6. Phase 5: release process and supply-chain gates.
7. Phase 6: threat model, abuse controls, smart-contract audit.
8. Phase 7: support/operations readiness review.

## Must-Answer Gate Before Production

- What launch mode is enabled, and what is explicitly disabled?
- What user action counts as available for each critical flow?
- Who is alerted for backend, frontend, worker, Solana, MagicBlock, KYC, and
  monitoring failures?
- How do we pause wagers, tournaments, settlement, or prize distribution?
- How do we recover from a bad deploy?
- How do we recover from a bad migration?
- How do we restore session and vault data?
- How do we prove who did an admin action?
- How do we prove which wallet signed or authorized a money-affecting action?
- How are duplicate/retried requests made safe?
- How are stale/failed background jobs retried safely?
- What happens if a Solana RPC, MagicBlock, Privy, email, DNS, Nginx, or VPS
  dependency fails?
- What support promise is made to users?
- Who can say "no launch"?

## First PR Slice

Recommended small first implementation PR:

1. Add `docs/LAUNCH_SCOPE.md` with launch mode and disabled paths.
2. Add backend config validation that fails closed in production.
3. Add `docs/ROUTE_AUTHORIZATION_MATRIX.md` generated by manual route inventory.
4. Add a CI secret scan.
5. Fix any doc links that point at moved or missing ops/monitoring paths.

This gives the production work a spine before changing high-risk runtime code.
