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

    /// Cycle Standard3D ⇄ Standard2D (TempleOS collapses back to 3D).
    ///
    /// `ViewMode` is the single source of truth for which board view is live;
    /// the piece-visibility system keys off `resource_changed::<ViewMode>`, so
    /// mutating it here is all that's needed — there is no second copy to sync.
    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::Standard3D => ViewMode::Standard2D,
            ViewMode::Standard2D => ViewMode::Standard3D,
            #[cfg(feature = "templeos")]
            ViewMode::TempleOS => ViewMode::Standard3D,
        };
    }
}
