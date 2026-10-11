# Program upgrades

Build/test the exact source being shipped and verify the current cluster, program,
upgrade authority and binary before submitting an upgrade. The records below
document lifecycle and Swiss compatibility; they do not establish today's deployed
state or that a recorded buffer still exists. See [release verification](../development/verification.md).

## Cancellation compatibility

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

## Deploy log
- **2026-10-09, devnet:** upgraded in slot 509278920 (sig `5rTpVxkh…AdE`) from commit
  `fe189f8` (binary sha256 `5d758705…b646de`, 1,337,056 bytes, 161/161 program tests
  against that binary). `verify_devnet_program.ps1` → MATCH. Live drill
  `tests/live_devnet_lifecycle.rs`: game 10891572912 created with a 0.002 SOL stake and
  cancelled before join — full refund, escrow empty, status `Cancelled`.

## Rollback
Redeploy the previous buffer/binary the same way. Rolling back reintroduces the two bugs
above; it does not touch account data.

---

## Swiss and tournament games (recorded 2026-10-10)

This change was recorded as pending deployment. Recheck program bytes and
instruction compatibility before enabling updated callers or reusing the buffer.

## What changes on chain
| Instruction | Change | Compatibility |
|---|---|---|
| `record_swiss_result` | Signed by `tournament.authority` (was any registered player); accounts `…, authority, white, black` (was `player, opponent, system_program`); result enum `WhiteWin/BlackWin/Draw/Bye`. Board cap is ⌈players/2⌉; a bye is a board slot after the real boards. | **Breaking** for old callers; the backend orchestrator and cu_benchmark use the new shape. |
| `advance_round` | Authority-signed (was any cranker); new arg `boards_in_round: u16`. | **Breaking.** |
| `complete_swiss_tournament` | Authority-signed; new arg `places: Vec<Pubkey>` (off-chain tiebreak order, checked against on-chain scores, error `InvalidSwissPlaces`). | **Breaking.** |
| `create_tournament_game` | **New.** Authority seats a pairing in an Active Game and creates both players' per-game `SessionDelegation`s → no player signature per game. | Additive. |

No account layout changes are recorded for this change. Inspect in-flight
tournaments on the selected cluster before replacing instruction contracts.

## Pre-flight (done 2026-10-10)
- Binary sha256 `0c82c1db5d4568d5d0c74286bd1563509d152d3d07147fbb67019303c32253a4`,
  1,354,464 bytes (fits the 1,424,064-byte ProgramData), 168/168 program tests.
- Buffer written to devnet: `F28H4dPcSNFSCQ9EyV5qoSCakyV48L4CSg7EiPMKBkXa`
  (authority `C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw`).

## Upgrade
```
solana program upgrade F28H4dPcSNFSCQ9EyV5qoSCakyV48L4CSg7EiPMKBkXa 8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU -u devnet
.\scripts\verify_devnet_program.ps1   # expect MATCH
```
To abandon instead, reclaim the buffer's rent: `solana program close F28H4dPcSNFSCQ9EyV5qoSCakyV48L4CSg7EiPMKBkXa -u devnet`.

## Verify (on-chain Swiss tournament, no backend)
`cargo run -p cu_benchmark -- --live --mode swiss --size 8` — pairs with `swiss-pairing`,
creates every game with `create_tournament_game` (checks it is Active with the right seats),
records results, advances each round, completes with the engine's places, distributes
prizes and closes the tournament.

Then redeploy the backend (it holds the new orchestrator, reconciler and game provisioner).
