#![warn(missing_docs)]

pub mod color;
pub mod error;
pub mod pairing;
pub mod standings;
pub mod types;

pub use color::*;
pub use error::*;
pub use standings::*;
pub use types::*;

pub use pairing::generate_pairings;
pub use standings::calculate_standings;
pub use types::{ManualPairing, PairingConfig};
