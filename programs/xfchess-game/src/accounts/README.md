# accounts

Shared `#[derive(Accounts)]` instruction contexts that don't belong to a single
instruction group. Account **data** structs live in [../state/](../state/README.md);
this directory only holds validation contexts.

## Files

| File | Contents |
|------|----------|
| [session_delegation.rs](session_delegation.rs) | `AuthorizeSessionKey` context — validates `Game` + `SessionDelegation` PDAs for session-key authorization |
| [move_batch.rs](move_batch.rs) | Placeholder for ER move-batch payload types (definitions currently live with session_delegation) |

## Example

```rust
// session_delegation.rs
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct AuthorizeSessionKey<'info> {
    // seeds-checked Game + init SessionDelegation PDA…
}
```
