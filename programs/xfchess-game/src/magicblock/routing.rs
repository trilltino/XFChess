//! Magic Router boundary notes.
//!
//! Delegated `Game` PDAs are writable on the ER and locked from base-layer
//! writes. Transactions that write escrow, profiles, treasury, or player
//! lamports must run only after the Game PDA is undelegated.

pub const GAME_WRITES_ONLY_ROUTING_INVARIANT: &str =
    "ER hot-path instructions write only the Game PDA";
