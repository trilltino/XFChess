//! Tournament match management instructions
//!
//! Instructions for match initialization, result recording, and Swiss system results.

pub mod guards;
pub mod initialize_match;
pub mod record_result;
pub mod record_swiss_result;

pub use initialize_match::InitializeMatch;
pub use record_result::{AdvanceWinner, RecordMatchResult};
pub use record_swiss_result::{RecordSwissResult, SwissMatchResult};
