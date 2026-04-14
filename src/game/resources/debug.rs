//! Debug logging throttle resource
//!
//! Prevents log spam by throttling frequent debug messages.
//! Only logs periodic summaries instead of every frame.

use bevy::prelude::*;

/// Resource to control debug log timing
#[derive(Resource, Debug)]
#[allow(dead_code)]
pub struct DebugThrottle {
    /// Last time we logged a periodic summary
    pub last_summary_time: f32,
    /// Interval for periodic summaries (seconds)
    pub summary_interval: f32,
    /// Enable verbose debugging (shows every event)
    pub verbose: bool,
}

impl Default for DebugThrottle {
    fn default() -> Self {
        Self {
            last_summary_time: 0.0,
            summary_interval: 5.0, // Summary every 5 seconds
            verbose: false,        // Set to true for detailed logs
        }
    }
}
