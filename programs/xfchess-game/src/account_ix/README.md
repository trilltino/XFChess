# account_ix

Account instructions manage profiles, usernames, player sessions, global sessions, fee vaults, friendships, and treasury withdrawals.

## Invariants

- `profile_init` preserves existing gameplay stats, verification flags, and external-link state.
- Identity fields are updated separately from gameplay fields.
- Session spending uses checked arithmetic and rejects overflow instead of saturating into an allowed value.
- External ELO values use the centiscale unit defined in `elo::rating`.
