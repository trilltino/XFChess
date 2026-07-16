# programs/xfchess-game — Solana program

Anchor 0.31 program holding all on-chain XFChess state: games, profiles, tournaments,
prize vaults, and MagicBlock Ephemeral Rollup (ER) delegation.
Program ID (localnet + devnet): `8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU`.

## Role in XFChess

The trust root for staked play: the backend builds unsigned transactions against this
program, clients sign them, and moves are validated on-chain by
[`chess-logic-on-chain`](../../crates/solana/chess-logic-on-chain/) (`move-validation`
feature). During a game the `Game` PDA is delegated to the ER for sub-second
`record_move`, then committed back for settlement — see [MAGICBLOCK.md](../../MAGICBLOCK.md)
and [docs/architecture/xfchess-game-crate.md](../../docs/architecture/xfchess-game-crate.md).

## Module map (src/)

| Module | Contents |
|--------|----------|
| [lib.rs](src/lib.rs) | Anchor entry point mapping every instruction to its handler |
| [state/](src/state/README.md) | All `#[account]` structs (Game, Tournament, vaults, sessions) |
| [game_ix/](src/game_ix/README.md) | create / join / resign / timeout / finalize |
| [moves_ix/](src/moves_ix/README.md) | `record_move` — runs on the ER |
| [delegation_ix/](src/delegation_ix/README.md) | ER delegate/undelegate + session keys |
| [tournament_ix/](src/tournament_ix/README.md) | Tournament lifecycle, registration, matches, prizes |
| [account_ix/](src/account_ix/README.md) | Profiles, fee vault, session keys, Elo updates |
| [governance_ix/](src/governance_ix/README.md) | Disputes and resolution |
| [crank_ix/](src/crank_ix/README.md) | ER scheduled time checks (`cranks` feature) |
| [lifecycle/](src/lifecycle/README.md) | Plain-Rust state machine: transitions, guards, settlement |
| [common/](src/common/README.md) | Escrow lamport-movement helpers |
| [magicblock/](src/magicblock/README.md) | ER CPI adapters and routing assumptions |
| [accounts/](src/accounts/README.md) | Shared `#[derive(Accounts)]` contexts (session auth) |
| [elo/](src/elo/README.md) | Centiscale Elo math |

## Build, test, deploy

```bash
scripts\build_program.bat        # or: anchor build (size-optimized, opt-level = "z")
cargo test -p xfchess-game       # all program tests (see tests/README.md)
anchor deploy                    # devnet
```

## Invariants

- Feature flags: `cranks` and `move-validation` are **default-on**; code touching them
  must compile with and without.
- Game accounts must be delegated before ER moves are recorded and undelegated before
  settlement; `process_undelegation` is invoked by ER infrastructure.
- State transitions and fund movement go through [lifecycle/](src/lifecycle/) and
  [common/escrow.rs](src/common/escrow.rs) respectively — never inline them in handlers.
