//! Debug logging throttle resource
//!
//! Prevents log spam by throttling frequent debug messages.
//! Only logs periodic summaries instead of every frame.

use bevy::prelude::*;

/// Resource to control debug log timing
#[derive(Resource, Debug)]
pub struct DebugThrottle {
    /// Last time we logged a periodic summary
    #[allow(dead_code)] // Reserved for future debug logging features
    pub last_summary_time: f32,
    /// Interval for periodic summaries (seconds)
    #[allow(dead_code)] // Reserved for future debug logging features
    pub summary_interval: f32,
    /// Enable verbose debugging (shows every event)
    #[allow(dead_code)] // Reserved for future debug logging features
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

impl DebugThrottle {
    /// Check if enough time has passed to log a summary
    #[allow(dead_code)] // Reserved for future debug logging features
    pub fn should_log_summary(&mut self, current_time: f32) -> bool {
        if current_time - self.last_summary_time >= self.summary_interval {
            self.last_summary_time = current_time;
            true
        } else {
            false
        }
    }

    /// Always log important events regardless of throttle
    #[allow(dead_code)] // Reserved for future debug logging features
    pub fn should_log_event(&self) -> bool {
        self.verbose
    }
}
