use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum ViewMode {
    #[default]
    Standard3D,
    /// TempleOS tribute theme — only compiled in dev builds (`--features templeos`).
    #[cfg(feature = "templeos")]
    TempleOS,
    Standard2D,
}

impl ViewMode {
    /// True when the TempleOS theme is active.
    /// Always false unless built with the `templeos` dev feature.
    #[inline]
    pub fn is_templeos(self) -> bool {
        #[cfg(feature = "templeos")]
        return self == ViewMode::TempleOS;
        #[cfg(not(feature = "templeos"))]
        return false;
    }
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
            #[cfg(feature = "templeos")]
            ViewMode::TempleOS => ViewMode::Standard3D, // TempleOS to 3D
        };
    }
}
