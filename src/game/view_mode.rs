use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum ViewMode {
    #[default]
    Standard3D,
    TempleOS,
    Standard2D,
}

/// Resource tracking player-specific view preferences
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerViewPreferences {
    pub local_view: ViewMode,
}

impl PlayerViewPreferences {
    pub fn toggle_view(&mut self) {
        self.local_view = match self.local_view {
            ViewMode::Standard3D => ViewMode::Standard2D,
            ViewMode::Standard2D => ViewMode::Standard3D,
            ViewMode::TempleOS => ViewMode::Standard3D, // TempleOS to 3D
        };
    }
}
