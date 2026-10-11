# System boundaries

| Layer | Responsibility | Entry point |
| --- | --- | --- |
| Native game | Bevy game state, legal local play, UI and multiplayer application | [src/lib.rs](../../src/lib.rs) |
| Desktop shell | Wallet signing, popup/game lifetime, optional operator window | [Tauri](../../tauri/src/README.md) |
| Backend | Auth, durable game log, social/tournament records, workers and transaction submission | [backend](../../backend/README.md) |
| Solana program | Wager/tournament authorization, game state and settlement rules | [program domains](xfchess-game-crate.md) |
| MagicBlock | Delegated game execution, commit and undelegation | [game lifecycle](magicblock-game-lifecycle.md) |
| Iroh/Braid | Peer delivery, ordered subscriptions and reconnect history | [multiplayer](networking.md) |
| Website | Browser account/player pages and release discovery | [website](../../xfchessdotcom/README.md) |
| Operations | Native services, proxy, monitoring and SQLite recovery | [ops](../../ops/README.md) |

Offline and casual gameplay do not require an ephemeral rollup. Wagered gameplay
distinguishes transport delivery, authoritative move acceptance and settled money.
Browser and peer messages cannot authorize payouts.

[Architecture decisions](../adr/README.md) preserve the reasons for program
invariants. [Security boundaries](security.md) describe privileged components;
component READMEs contain module-level details.
