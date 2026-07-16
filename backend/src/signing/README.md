# backend/src/signing

The backend's core domain module: JWT/wallet auth, Solana transaction building,
HTTP route handlers, compliance checks, P2P relay state, and tournament storage.
"Signing" refers to co-signing session/relayer flows — **player keys never touch
this server**; player-signed transactions are built here unsigned and returned.

## Submodules

| Path | Contents |
|------|----------|
| [routes/](routes/README.md) | All HTTP handlers, one file per API area (auth, matchmaking, tournament, puzzle, kyc, …) |
| [solana/](solana/) | Instruction building, RPC clients, base-vs-ER routing, tx debug endpoint |
| [blinks/](blinks/) | Solana Actions/Blinks: core spec types, funding, onboarding, anti-cheat, PDA helpers |
| [cacf/](cacf/) | Jurisdiction compliance: [uk.rs](cacf/uk.rs), [brazil.rs](cacf/brazil.rs), [germany.rs](cacf/germany.rs), [canada.rs](cacf/canada.rs) |
| [p2p_relay/](p2p_relay/) | Iroh relay session state per game ID |
| [social/](social/) | Friends, presence, chat routes |
| [storage/](storage/) | SQLite-backed stores: [tournament.rs](storage/tournament.rs) (state of record), sessions, identity vault |
| [swiss/](swiss/) | Swiss tournament orchestration over `crates/shared/swiss-pairing` ([SCORING.md](swiss/SCORING.md)) |

Root files: [auth.rs](auth.rs)/[auth_ws.rs](auth_ws.rs) (JWT + WebSocket auth),
[config.rs](config.rs) (`SigningConfig`), [feepayer.rs](feepayer.rs) (relayer fee-payer
pool), [identity.rs](identity.rs) (encrypted KYC vault), [pyth_oracle.rs](pyth_oracle.rs)
(SOL/USD rates), [ws_subscriber.rs](ws_subscriber.rs) (live game subscription,
`ws_subscriber` feature), [tee_relayer.rs](tee_relayer.rs),
[blinks_funding.rs](blinks_funding.rs) / [blinks_onboarding.rs](blinks_onboarding.rs)
(route-level blinks endpoints), [elo_cache.rs](elo_cache.rs), [linkage.rs](linkage.rs),
[anticheat_enqueue.rs](anticheat_enqueue.rs).

## Example

```
Client                         signing/                         Solana
  │ POST /session/create  ──►  routes/main.rs
  │                            solana/transactions.rs builds unsigned tx
  │ ◄── serialized tx ────────
  │ signs locally, returns ──► routes co-sign with session key ──► RPC / Magic Router
```

## Invariants

- Wager/staked routes must call the [cacf/](cacf/) check before building a transaction.
- Anything touching live tournaments goes through `storage/tournament.rs` — no
  parallel in-memory tournament state.
- `AppState` (defined in [mod.rs](mod.rs)) is the only shared-state struct; route
  files take it via `State<AppState>`.
