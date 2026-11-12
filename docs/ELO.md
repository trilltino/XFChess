# On-chain Elo

Ratings live in `PlayerProfile` and change only when a rated game is settled
([settlement.rs](../programs/xfchess-game/src/lifecycle/settlement.rs)). All maths is integer; there are no floats on-chain.

## Storage

- Four ratings: bullet, blitz, rapid, classical. Each is a `u32` in hundredths of a point (1500.00 is `150_000`). A rating of 0 means unplayed and reads as 1200.
- Time control picks the bucket: under 3 min bullet, 3 to under 10 min blitz, 10 to under 25 min rapid, otherwise (including no clock) classical.
- Per bucket, a `u8` count of rated games (saturates at 255), plus the last rated opponent (8 bytes) and a repeat counter.
- Fixed-size fields come first and the three strings last, so numeric offsets are the same in every account. `elo_cache.rs` in the backend decodes by offset and has a layout test against the program's own struct.

## Update rule ([elo/](../programs/xfchess-game/src/elo/))

1. Expected score comes from a table (rating gap 0 to 800 in 10-point steps, linear interpolation between), so no `powf`. Gaps beyond 800 use the last entry. A unit test keeps it within 0.001 of the float formula.
2. K depends on the player: 40 for the first 30 rated games in that bucket, then 20, and 10 from 2400.
3. Change = K x (score - expected), scaled by a weight (below). Ratings never fall below 100.0.

## Who gets rated

- Only `MatchType::Rated` games. Free games, and tournament games (which are created as `Free`), do not change ratings. Results, wins, losses and `games_played` always count.
- The wager must be at least `MIN_RATED_WAGER_LAMPORTS` (0.001 SOL). Below it the game settles and counts toward the record but moves no rating. This is a product value: tune it before mainnet.

## Repeat-opponent weight ([repeat.rs](../programs/xfchess-game/src/elo/repeat.rs))

Each profile remembers its last rated opponent. Consecutive rated games against the same opponent: 1st-3rd count fully, 4th 50%, 5th 25%, 6th onward 0%. Playing someone else resets it. This blunts two-wallet win trading. It does not stop a ring of three or more wallets.

## Seeding from Lichess

The first successful `link_external_elo` sets bullet, blitz and rapid to 1200 plus three quarters of the distance to the linked Lichess rating, clamped to 800-2200. Only buckets with no rated games are seeded, and only once (`seeded_from_external`). Later syncs only refresh the stored Lichess values. Classical is not seeded.

## Not done

- The profile is 262 bytes, not the 64 the cost plan targets: country, date of birth, annual winnings and the Lichess fields are still on-chain.
- Closing a profile and reactivating it with a rating snapshot (plan section on profiles).
- Matchmaking uses the classical rating whatever the time control.
- No decay for inactive players, and rating changes cannot be reversed if anti-cheat later voids a result.
