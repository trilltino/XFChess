//! Serde-only DTOs shared between the backend and the web frontend (`xfchessdotcom`)
//! over JSON. Free of Bevy, Solana SDK, and async-runtime dependencies so the
//! web-facing API surface can evolve independently of the game client toolchain.
//! See `crates/shared/backend-types/README.md` for the crate's module layout and
//! field-change contract.

pub mod tournament;
