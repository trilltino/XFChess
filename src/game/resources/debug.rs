use bevy::prelude::*;

#[derive(Resource, Debug)]
pub struct DebugThrottle {
    pub last_summary_time: f32,
    pub summary_interval: f32,
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
