# delegation_ix

Delegates `Game` PDAs to MagicBlock Ephemeral Rollups (ER) for sub-second move
recording, commits/undelegates them back to the base layer at game end, and manages
per-game session keys for passwordless play. Full lifecycle: [MAGICBLOCK.md](../../../../MAGICBLOCK.md).

## Files

| File | Instructions |
|------|--------------|
| [delegate.rs](delegate.rs) | `handler_delegate_game` / `handler_undelegate_game` — ER delegate + commit-and-undelegate CPIs via `ephemeral_rollups_sdk` |
| [session.rs](session.rs) | `handler_authorize_session_key` / `handler_revoke_session_key` — per-game `SessionDelegation` (2h expiry, batch cap) |
| [undelegation.rs](undelegation.rs) | `InitializeAfterUndelegation` — called by ER infrastructure when restoring the account |

## Example

```rust
// delegate.rs — state must be mutated BEFORE the CPI flips the account owner
crate::lifecycle::transitions::mark_delegated(&mut game)?;
game.try_serialize(&mut writer)?;
drop(game_data);
crate::magicblock::delegation::delegate_game_pda(delegate_accounts, &game_id_bytes)?;
```

## Invariants

- `Game` accounts use **manual** deserialize/serialize (`AccountInfo`, not `Account<Game>`)
  because the delegation CPI changes the account owner; Anchor's exit serialization would
  conflict. Mutate and re-serialize before the CPI, and `drop` the data borrow first.
- Delegation passes `validator: None` — the magic-router assigns a validator. Never pin
  a validator pubkey (it breaks mainnet and forces one region).
- `handler_undelegate_game` deliberately skips a payer identity check so the session key
  can trigger it without a wallet popup at game end.
- `_valid_until` on delegate is retained for ABI compatibility only; it is not used.
- Lifecycle transitions go through `crate::lifecycle::transitions::{mark_delegated, mark_undelegated}` —
  do not set `is_delegated` directly.
