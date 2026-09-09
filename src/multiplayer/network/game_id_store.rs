use std::sync::atomic::{AtomicU64, Ordering};

static ACTIVE_GAME_ID: AtomicU64 = AtomicU64::new(0);

pub fn set(id: u64) {
    ACTIVE_GAME_ID.store(id, Ordering::SeqCst);
}

pub fn get() -> u64 {
    ACTIVE_GAME_ID.load(Ordering::SeqCst)
}
