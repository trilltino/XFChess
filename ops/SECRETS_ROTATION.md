# Secret recovery and rotation

The supported backend configuration is [backend/.env.example](backend/.env.example)
and `SigningConfig` in `backend/src/signing/config.rs`. Production deployment
preserves `/opt/xfchess/.env` and never uploads operator private keys implicitly.
Store a recoverable copy in an encrypted offline store, with restricted access.

| Secret | Rotation constraint |
| --- | --- |
| `ADMIN_API_KEY` | Update the server and the panel's saved OS credential; restart the backend. |
| `JWT_SECRET` | Existing login tokens become invalid; users must sign in again. |
| `RELAY_SHARED_SECRET` | Coordinate backend and trusted game clients. |
| `FEE_PAYER_KEYS` | Fund/verify new fee payers on the selected cluster before removing old ones. |
| `VPS_AUTHORITY_KEY`, `KYC_AUTHORITY_KEY` | Verify program authority constants and active delegations first; changing an environment value alone does not rotate on-chain authority. |
| `IDENTITY_ENCRYPTION_KEY`, `IDENTITY_SALT` | Preserve both. Changing them prevents decryption of existing identity/session records and backups without a validated re-encryption migration. |
| Backup age identity | Keep the private identity offline; changing the public recipient affects future backups only. Retain old identities for retained objects. |

The program currently uses authority constants in
`programs/xfchess-game/src/constants.rs`. There is no generic authority-rotation
CLI or automatic identity re-encryption migration in this runbook; inspect and
test the actual program upgrade path before changing those keys.

After a supported rotation, restart the backend, verify private `/readyz` and
JSON `/health`, test the affected authenticated operation, and update the offline
recovery record. Retire old material only after confirming retained backups can
still be restored. Record public identifiers and dates, never private values.
