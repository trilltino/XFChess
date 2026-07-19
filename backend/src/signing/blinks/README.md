# backend/src/signing/blinks

Solana Actions/Blinks implementation (the shareable-link transaction spec): action
metadata endpoints and transaction building for tournament registration, onboarding,
and funding.

## Files

| File | Contents |
|------|----------|
| [core.rs](core.rs) | Action metadata + tournament-registration transaction building |
| [routes.rs](routes.rs) | Axum routes serving the Actions spec (GET metadata / POST transaction) |
| [anti_cheat.rs](anti_cheat.rs) | Pre-sign validation (IP-based checks before returning a transaction) |
| [chains.rs](chains.rs) | Action chaining for multi-step flows |
| [funding.rs](funding.rs) / [onboarding.rs](onboarding.rs) | Wallet funding and first-time-user actions |
| [pda.rs](pda.rs) | PDA derivation helpers for action targets |

## Invariants

- Responses must stay conformant to the Solana Actions spec (wallets validate CORS
  and `actions.json`).
- Like every other route: transactions are returned **unsigned**.
