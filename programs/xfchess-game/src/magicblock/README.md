# magicblock

Adapters around the MagicBlock ER SDK: the CPI call sites and routing assumptions in
one place, so instruction handlers don't spread SDK details. ADR:
[docs/adr/0002-magic-router-routing.md](../../../../docs/adr/0002-magic-router-routing.md).

## Files

| File | Contents |
|------|----------|
| [delegation.rs](delegation.rs) | `delegate_game_pda`, `commit_and_undelegate_game_pda` — wraps `ephemeral_rollups_sdk::cpi` |
| [crank.rs](crank.rs) | `build_time_check_schedule_instruction` — scheduled-task instruction builder for [crank_ix/](../crank_ix/README.md) |
| [routing.rs](routing.rs) | Which cluster a given operation must target (base vs ER) |

## Example

```rust
// delegation.rs is the only place the ER delegation CPI is invoked:
crate::magicblock::delegation::delegate_game_pda(delegate_accounts, &game_id_bytes)?;
```

## Invariants

- Delegation never pins a validator (`validator: None`); the magic-router assigns one.
- Callers must finish all account mutation **before** these CPIs (they change the
  account owner) — see [delegation_ix/README.md](../delegation_ix/README.md).
