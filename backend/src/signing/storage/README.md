# backend/src/signing/storage

SQLite-backed stores for the signing service's durable state.

## Files

| File | Contents |
|------|----------|
| [tournament.rs](tournament.rs) | `TournamentStore` — **the source of truth for live tournaments**: one JSON blob per record in the `tournaments` table; survives restarts |
| [session.rs](session.rs) | `SessionStore` — game session keys and status |
| [vault.rs](vault.rs) | Encrypted identity/KYC vault (GDPR: encrypted at rest, deletable) |

## Invariants

- All tournament mutations go through `TournamentStore` — no parallel in-memory
  tournament state anywhere in the backend.
- Vault contents are encrypted with `IDENTITY_ENCRYPTION_KEY`; rotating that key
  requires the re-encryption migration first (see
  [deploy/SECRETS_ROTATION.md](../../../../deploy/SECRETS_ROTATION.md)).
