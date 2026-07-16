# state

Account structs (`#[account]` + `InitSpace`) for every PDA the program owns. One file
per account type; ADR-0004 covers tournament shard invariants
([docs/adr/0004-tournament-shard-invariants.md](../../../../docs/adr/0004-tournament-shard-invariants.md)).

## Accounts

| File | Account | Seeds / notes |
|------|---------|---------------|
| [game.rs](game.rs) | `Game` | `[b"game", game_id]` — compact 68-byte `board_state` (not FEN), clocks, wager, `is_delegated`, replay `nonce` |
| [game.rs](game.rs) | `SessionDelegation` | `[b"session_delegation", game_id, player]` — per-game hot key, expiry, batch cap |
| [global_session.rs](global_session.rs) | `GlobalSessionDelegation` | Wallet-wide session key (one popup ever) |
| [tournament_session.rs](tournament_session.rs) | `TournamentSessionDelegation` | Tournament-scoped session key |
| [player_profile.rs](player_profile.rs) | `PlayerProfile` | Elo (centiscale, see [../elo/README.md](../elo/README.md)), stats |
| [player_session.rs](player_session.rs) | `PlayerSession` | Matchmaking session record |
| [tournament.rs](tournament.rs) | `Tournament`, `VestingParams`, `SwissStanding`, `TournamentPlayersShard` | Single-elimination (8–256 players, power of 2) **and** Swiss (`total_rounds`, standings); top-10 prize split in basis points |
| [tournament_match.rs](tournament_match.rs) | `TournamentMatch` | One match within a tournament round |
| [username_record.rs](username_record.rs) | `UsernameRecord` | Username → wallet mapping |
| [friendship.rs](friendship.rs) | `Friendship` | On-chain friends graph |
| [dispute.rs](dispute.rs) | `DisputeRecord` | Governance dispute state |
| [platform_fee_vault.rs](platform_fee_vault.rs) / [treasury_vault.rs](treasury_vault.rs) | `PlatformFeeVault`, `TreasuryVault` | Fee and treasury lamport vaults |

## Example

```rust
/// The core on-chain game account. One PDA per game_id.
/// Seeds: [b"game", game_id.to_le_bytes()]
#[account]
#[derive(InitSpace)]
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,          // default pubkey = no opponent yet
    pub status: GameStatus,     // Pending → Active → Finished → Settled …
    pub board_state: [u8; 68],  // compact binary board, replaces FEN
    pub is_delegated: bool,     // true while owned by the MagicBlock ER
    // …
}
```

## Invariants

- `GameStatus` is the single lifecycle enum (`Pending … Settled/Expired/Cancelled`);
  transitions are enforced in `src/lifecycle/`, not ad hoc in instructions.
- `Game.turn` is `u16` on purpose — a `u8` overflows past half-move 255.
- Tournaments are both single-elimination and Swiss; Swiss pairing itself happens
  off-chain (`crates/shared/swiss-pairing`), the program records results and standings.
- Struct layout is ABI: add fields only at the end, and only with a coordinated
  program + client migration.
