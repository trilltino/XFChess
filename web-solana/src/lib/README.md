# web-solana/src/lib

Client libraries: backend API access, Anchor program client, and MagicBlock ER
helpers.

## Layout

| Path | Contents |
|------|----------|
| [api/client.ts](api/client.ts) | Shared HTTP client; base URL from `VITE_BACKEND_URL` |
| [api/](api/) | Per-area wrappers: [auth.ts](api/auth.ts), [games.ts](api/games.ts), [tournament.ts](api/tournament.ts), [kyc.ts](api/kyc.ts), [lichess.ts](api/lichess.ts) |
| [anchor_client.ts](anchor_client.ts) | Anchor `Program` construction from the `xfchess-game` IDL |
| [magicblock.ts](magicblock.ts) | `MagicBlockClient` — dual connections (base layer + ER), `isDelegated`, `getProgramForDelegated`, `executeOnDelegated` (see [../../MAGICBLOCK_SETUP.md](../../MAGICBLOCK_SETUP.md)) |
| [useKycStatus.ts](useKycStatus.ts) | KYC status hook |
| [countryStablecoins.ts](countryStablecoins.ts) | Country → stablecoin mapping for funding |
| [tauriNotifications.ts](tauriNotifications.ts) | Notifications when running inside the desktop wrapper |

## Example

```ts
// magicblock.ts — delegated games are read/written via the ER connection
const client = new MagicBlockClient(wallet);
const program = await client.getProgramForDelegated(gamePda); // ER or base automatically
```

## Invariants

- The IDL consumed by [anchor_client.ts](anchor_client.ts) must be regenerated
  (`anchor build`) whenever program instructions change.
- ER operations use `skipPreflight: true`; base-layer operations must not.
