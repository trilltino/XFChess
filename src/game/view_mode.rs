use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Resource, Default)]
#[reflect(Resource)]
pub enum ViewMode {
    #[default]
    Standard3D,
    #[cfg(feature = "templeos")]
    TempleOS,
    Standard2D,
}

impl ViewMode {
    #[inline]
    pub fn is_templeos(self) -> bool {
        #[cfg(feature = "templeos")]
        return self == ViewMode::TempleOS;
        #[cfg(not(feature = "templeos"))]
        return false;
    }

    pub fn toggle(&mut self) {
        *self = match *self {
            ViewMode::Standard3D => ViewMode::Standard2D,
            ViewMode::Standard2D => ViewMode::Standard3D,
            #[cfg(feature = "templeos")]
            ViewMode::TempleOS => ViewMode::Standard3D,
        };
    }
}
