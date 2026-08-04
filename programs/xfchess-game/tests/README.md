# programs/xfchess-game/tests

Integration tests for the Solana program, run with `cargo test -p xfchess-game`.
Shared fixtures (program setup, funded keypairs, PDA helpers) live in
[common/mod.rs](common/mod.rs).

## Test files

| File | Covers |
|------|--------|
| [game_settlement_tests.rs](game_settlement_tests.rs) | Finalize/settlement: pot payout, Elo, profile stats |
| [er_delegation_tests.rs](er_delegation_tests.rs) / [er_move_tests.rs](er_move_tests.rs) | ER delegate/undelegate lifecycle and `record_move` |
| [tournament_registration_tests.rs](tournament_registration_tests.rs) / [tournament_match_tests.rs](tournament_match_tests.rs) / [tournament_prize_tests.rs](tournament_prize_tests.rs) | Tournament lifecycle end to end |
| [treasury_tests.rs](treasury_tests.rs) | Fee vault + treasury withdrawal |
| [dispute_tests.rs](dispute_tests.rs) | Governance dispute/resolve/claim-stale |
| [profile_session_tests.rs](profile_session_tests.rs) | Profile init + session key auth |

## Running

```bash
cargo test -p xfchess-game                            # all
cargo test -p xfchess-game --test treasury_tests      # one file
```
