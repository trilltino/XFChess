# Game program upgrade (multiplayer lifecycle fixes, October 2026)

**Symptom:** players cannot cancel a wagered game before an opponent joins ("can't cancel a
game before an opponent joins yet … reclaimed here in about Nh"), or an outsider could
cancel an abandoned active game. Both are fixed in source but not live until this upgrade.
**Severity:** S2 (funds safe in escrow, recoverable after 24h via `withdraw_expired_wager`).

## What changes on chain
| Instruction | Change | Compatibility |
|---|---|---|
| `cancel_game` | `black_authority` is `UncheckedAccount` with the same `== game.black` constraint, no Anchor `mut`; writability is required in the handler only when black has joined. Unjoined games were always rejected before (3011 `AccountNotSystemOwned`, because `game.black` is the System Program id). | Same accounts, same order, same data. Existing clients already pass the account writable. Anchor's generated metas now mark it read-only, so an IDL-generated client must set it writable for joined games (the handler fails with `ConstraintMut` otherwise). |
| `cancel_game` | Active-game cancellation requires the signer to be white or black for every move count (an outsider could cancel an idle game after 24h). | No account or data change. |

No account layout changes; no migration. In-flight games are unaffected.

## Pre-flight (read-only)
1. Build from the exact commit being shipped: `just build-program`.
2. Run the program suite against that binary (copy the `.so` to `target\deploy\`):
   `cargo test -p xfchess-game --no-fail-fast` — expect 0 failures, including
   `lifecycle_race_tests` (8) and `claim_timeout_tests`.
3. Record the local hash and confirm what is currently deployed:
   `.\scripts\verify_devnet_program.ps1` (expect `MISMATCH` before the upgrade).
4. `solana program show 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU -u <cluster>` — note the
   upgrade authority and the ProgramData size. The new `.so` must fit, or extend first
   (`solana program extend`).

## Upgrade (a deliberately approved operator step — never part of an audit or CI)
1. Write a buffer: `solana program write-buffer target\deploy\xfchess_game.so -u <cluster>`.
2. If the upgrade authority is a multisig, transfer the buffer authority to it
   (`solana program set-buffer-authority <BUFFER> --new-buffer-authority <MULTISIG>`) and
   propose the upgrade there; otherwise:
   `solana program upgrade <BUFFER> 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU -u <cluster>`.
3. Re-run `.\scripts\verify_devnet_program.ps1` — expect `MATCH`.

## Verify (post-upgrade drills)
1. Create a small wagered game and cancel it before anyone joins → refund confirmed, status
   `Cancelled`, escrow empty; the settlement worker logs "cancelled before an opponent joined"
   and retires the session.
2. Create, join, cancel before the first move → both stakes refunded, no winner.
3. From a third wallet, try `cancel_game` on an idle active game → `NotInGame`.

## Rollback
Redeploy the previous buffer/binary the same way. Rolling back reintroduces the two bugs
above; it does not touch account data.
