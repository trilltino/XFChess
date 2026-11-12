//! Thin host crate for the XFChess client.
//!
//! The reusable application lives in `xfchess-client-bevy`; this package owns
//! executable and Android entry points only.

#[cfg(target_os = "android")]
pub mod android;

pub use xfchess_client_bevy::*;
