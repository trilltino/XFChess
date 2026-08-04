//! Cross-cutting helpers shared by instruction handlers.
//!
//! Currently this is the single source of truth for moving lamports, so the
//! "system-owned PDA ⇒ signed CPI transfer" vs "program-owned PDA ⇒ direct
//! debit" distinction lives in exactly one place. See [`escrow`].

pub mod escrow;
