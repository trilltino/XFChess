use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PersistentEguiCamera {
    pub entity: Option<Entity>,
}
