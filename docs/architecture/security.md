# Security boundaries

Review changes against these boundaries. This describes design and review
obligations; it does not certify a deployment or its credentials.
Report vulnerabilities through [SECURITY.md](../../SECURITY.md).

| Asset or boundary | Required protection |
| --- | --- |
| Wager/tournament escrow | Program-enforced signer, participant, status and checked amount constraints; canonical settlement |
| Session authority | Scope, expiry and nonce checks; separate player permission and backend fee-payer authority |
| Peer/game events | Admitted identities, causal parents and legal application; acknowledgements do not prove settlement |
| Backend authorities | Keys outside source; production placeholder rejection; rotation and recovery access |
| Identity database | Preserve vault key/salt separately from backups; field encryption does not encrypt the whole SQLite file |
| Desktop wallet | Popup session/request identity, origin checks, cancellation and game-owned lifetime; no wallet shell capability |
| Operator panel | OS credential storage, private backend tunnel and bounded `deploy` SSH console |
| Web/API | Server authorization, revocation, configured CORS/rate limits; untrusted browser input |
| Deployment/backup | Committed releases, verified health, encrypted snapshots and stopped-service restore |

The backend holds fee-payer and privileged game/tournament keys. Review those
permissions independently: absence of player private keys does not remove the
backend's influence on funds or outcomes. Anti-cheat, KYC and country restrictions
need product-specific validation.

The public proxy blocks admin/metrics. Host binding, SSH policy and monitoring
configuration must also be applied on the server. An unset CORS origin list has
a permissive development fallback; deployment configuration matters.

Before real-money release, verify secret rotation, program bytes, client/IDL
compatibility, recovery drills and payout timing. Keep keys, tokens, identity
records and unredacted logs out of issues/PRs. See
[secret rotation](../../ops/SECRETS_ROTATION.md), [ops](../../ops/OPERATIONS.md) and
[verification](../development/verification.md).
