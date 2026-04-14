//! Gameplay instructions handling chess moves during an active match.

pub mod commit_batch;
pub mod record;

pub use commit_batch::CommitMoveBatchCtx;
pub use record::RecordMove;
