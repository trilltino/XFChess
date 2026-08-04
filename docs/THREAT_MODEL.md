# Threat Model — XFChess

Attacker/abuse/fraud paths and their mitigations, across the whole system (game,
web, desktop, backend, contracts, admin, networking). Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-F. Checklist §15, §28.

Money is on-chain and the stakes are real, so the model centres on: **can an attacker
take funds, forge results, impersonate a user, or take over the box?**

## Trust boundaries
```
 Player wallet ──signs──▶ Solana program  ◀──reads/writes── Backend (VPS, holds fee-payer + authority keys)
      │                        ▲                                   ▲
      │ P2P (Iroh/QUIC)        │ on-chain move validation          │ nginx (TLS, rate limit, CORS)
      ▼                        │                                   │
 Opponent client ──────────────┘                          Web / Desktop (Privy) clients
```
- **On-chain program** = source of truth for wagers, results, ELO. Backend cannot move
  funds it isn't authorized for; it never holds player keys.
- **Backend** holds fee-payer + VPS/KYC/dispute authority keys — the crown jewels.
- **Clients** are untrusted; anything security-relevant is enforced server-side or on-chain.

## Assets → threats → mitigations

| Asset | Threat | Mitigation | Status |
|---|---|---|---|
| Wager escrow | Forged game result to steal pot | Moves validated **on-chain** (`chess-logic-on-chain`); result derived from on-chain state; settlement reads the PDA | ✅ |
| Wager escrow | Replay/duplicate settlement | `finalize_game` idempotent on-chain; settlement worker re-derives from chain | ✅ |
| Session keys | Stolen session key signs moves for a game | Session key scoped per-game via `SessionDelegation` PDA; no funds authority; rotate via [SECRETS_ROTATION.md](../ops/SECRETS_ROTATION.md) | ✅ |
| Auth JWT | Token theft → impersonation | Server-side verify; short TTL; sig-replay window; revocation table (`017_jwt_revocations`) | ✅ (web JWT→httpOnly cookie still open, FRONTEND_REMEDIATION F2) |
| **Desktop wallet JWT** | Any website reads it from local bridge :7454 | CORS locked to localhost/tauri origins (Tauri T1); **full fix (per-launch bearer token) pending** | 🟡 partial |
| Authority keys (fee-payer/KYC/dispute) | Key leak → drain fees / forge KYC / resolve disputes | Keys only in untracked `.env` (chmod 600), hardened systemd, rotation runbook; **rotate the values leaked in git history before mainnet** | 🟡 rotate pending |
| Identity/KYC (vault.db) | DB exfiltration → PII leak | Encrypted fields (AES-GCM); offsite backups age-encrypted; at-rest DB encryption is P2 | 🟡 |
| VPS root | RCE via admin tooling | SSH shell scoped to tournament-admin window only (Tauri T2/T3); UFW; consider moving admin ops to an authed backend API | 🟡 |
| Solana RPC | Provider outage / throttle stalls settlement | Triton primary + fallback + circuit breaker + 30s timeouts (WS-D) | ✅ |

## Abuse / fraud (checklist §28)

| Abuse | Vector | Mitigation | Status |
|---|---|---|---|
| Cheating (engine assistance) | Player uses a chess engine | Anti-cheat Stockfish analysis workers + think-time telemetry; verdicts gate prize payout | ✅ |
| Sybil / collusion | Many accounts to farm/throw wagers | Account-linkage detection (`016_account_linkage`); IP anti-cheat; KYC for cash play | ✅ partial |
| Bulk account creation | Script mass signups (email/waitlist spam, disk fill) | nginx `mail` zone (3/min) + `auth` zone; email deduped per day; profile creation costs on-chain rent | ✅ |
| Credential stuffing / reset abuse | Automated login attempts | nginx `auth` rate zone; wallet-signature auth (no password to stuff) for the main path | ✅ partial |
| API scraping | Harvest player/leaderboard data | nginx `api` zone (120/min) + per-endpoint zones | ✅ |
| Compliance evasion | Play cash games from a restricted country | CACF checks (UK/BR/DE/CA) before building wager tx; KYC gate | ✅ |
| Email bombing | Enqueue floods of emails to a victim | Rate-limited endpoints + per-(template,email,day) dedupe in the job queue | ✅ |

## OWASP Top-10 quick pass (backend + web)
- **Broken access control** — object/tenant checks server-side; admin routes behind API key; audit admin actions (ADMIN, pending).
- **Crypto failures** — TLS in transit; identity fields encrypted; backups encrypted; **at-rest DB encryption P2**.
- **Injection** — SQLx parameterized queries throughout (no string-built SQL); input validation on email/username.
- **Insecure design** — funds on-chain, clients untrusted, server authoritative.
- **Security misconfig** — nginx headers/CSP, `server_tokens off`, monitoring bound to localhost (VPS audit R5/R8).
- **Vulnerable components** — CI `cargo audit` + `npm audit` (WS-F); triage the Solana/Iroh tree.
- **Auth failures** — wallet-sig auth, JWT TTL + revocation, sig-replay window.
- **Data integrity** — on-chain validation; artifact signing + SBOM are supply-chain follow-ups.
- **Logging/monitoring** — structured logs + correlation IDs (WS-E), Prometheus + alerts→runbooks (WS-H).
- **SSRF** — backend only calls known providers (RPC, Resend, CoinGecko/Frankfurter); no user-supplied fetch URLs.

## Top open items (tracked elsewhere)
1. Rotate secrets leaked in git history before mainnet — [E2E_REMEDIATION.md R1](../ops/docs/E2E_REMEDIATION.md).
2. Desktop bridge per-launch bearer token — [TAURI_REMEDIATION.md T1](../tauri/docs/TAURI_REMEDIATION.md).
3. Web JWT → httpOnly cookie — [FRONTEND_REMEDIATION.md F2](../ops/docs/FRONTEND_REMEDIATION.md).
4. At-rest DB encryption (SQLCipher) — P2.
5. Admin-action audit log + break-glass — WS component, pending.
