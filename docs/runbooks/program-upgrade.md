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

---

## Audit fixes (recorded 2026-10-16, pending deployment)

No account layout, discriminator, PDA seed, argument or account-list changes.
Every change narrows or corrects behaviour, so existing callers keep working;
the table notes where a caller relied on the old behaviour.

| Instruction | Change | Compatibility |
|---|---|---|
| `claim_timeout`, `crank_time_check` | A timed game times out only after the side to move is idle longer than its largest possible clock (base + increment per move made, minimum 90s); previously a fixed 90s, which forfeited legal thinks in 5- and 10-minute games. Zero-move (90s) and untimed (24h) windows are unchanged. | Claims between 90s and the clock bound now fail `TimeoutNotExpired`. |
| `crank_time_check` | Removed the `CrankTooLate` check, which compared a slot height with `unix_timestamp / 400` and rejected every run once the ER's slot height passed ~4.5M. | `CrankTooLate` stays in the error enum (codes unchanged) but is no longer returned. |
| `record_move` | A move by the player a draw offer was made to clears `draw_offered_by`; the offer could previously be accepted moves later. | None. |
| `undelegate_game` | While the game is not terminal, the signer must be `game.fee_payer`, `white` or `black` (`UnauthorizedAccess`). Each undelegation charges `UNDELEGATE_COST + ER_SESSION_FEE_LAMPORTS` to the pot. | The backend signs with the session key (`fee_payer`); cu_benchmark signs as white. |
| `request_force_undelegate` | Refused (`GameNotStuckDelegation`) unless the committed `Game` is terminal or idle past its timeout window. | The backend only requests for games the ER cannot serve. |
| `session_join_game` | The game must belong to the session's tournament and the tournament must be Active (`InvalidSession` / `InvalidTournamentStatus`). | No production caller (only unused cu_benchmark builders). |
| `complete_swiss_tournament` | Every shard the tournament's capacity needs must be passed (`InvalidSwissPlaces`). | The backend passes `required_shards(max_players)`. |
| `fund_sol_prize` | The operator must be the tournament authority (`NotTournamentAuthority`). | Backend, client and cu_benchmark fund as the authority. |
| `register_player` | Eligibility and seeding use the profile's rating for the tournament's time control; the `elo` argument is ignored. | Clients sent a constant 1200 (Blinks sent 0), so ELO-gated events previously checked a made-up number. |
| `finalize_game` | A wagered game whose escrow holds less than both stakes fails `InsufficientFunds` instead of closing the Game PDA without paying. | Unreachable through the program's own deposit paths. |

Pre-flight on 2026-10-16 (local, not a deployment): `cargo build-sbf` with
platform-tools v1.52 produced 1,343,168 bytes, sha256
`96d8fc95d2a8ad5537b2e9271b983e9e47f423b6343d02ca6dca6fcc1fbe2ba9`; the program
suite passed against it. Rebuild and re-record from the exact commit you ship.
Check that the binary fits the deployed ProgramData before writing a buffer.
