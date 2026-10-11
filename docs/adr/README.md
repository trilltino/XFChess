# Architecture decisions

Accepted decisions record the invariants contributors must preserve.

| Decision | Invariant |
| --- | --- |
| [0001](0001-split-terminal-result-from-settlement.md) | Separate terminal results from base settlement |
| [0002](0002-magic-router-routing.md) | Centralize delegated/base transaction routing |
| [0003](0003-canonical-game-settlement.md) | Keep payout/profile mutation in one settlement path |
| [0004](0004-tournament-shard-invariants.md) | Share shard validation and player-vector semantics |
| [0005](0005-profile-init-is-not-profile-reset.md) | Preserve existing gameplay/link data during profile initialization |

Add a decision when a durable architectural choice needs a rationale. Execution
plans, test logs and ordinary implementation changes belong in issues and Git.
