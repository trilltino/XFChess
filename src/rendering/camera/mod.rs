//! Camera presets and the TempleOS camera.

pub mod camera_director;
#[cfg(feature = "templeos")]
pub mod camera_templeos;

use bevy::prelude::*;

// Re-export all public items
#[cfg(feature = "templeos")]
pub use camera_templeos::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        use crate::game::systems::camera::view_mode_toggle_input_system;
        use bevy::input::common_conditions::input_just_pressed;
        // V switches 2D/3D, in a game only.
        app.add_systems(
            Update,
            view_mode_toggle_input_system
                .run_if(input_just_pressed(KeyCode::KeyV))
                .in_set(crate::core::schedule::GameSystems::Input),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::states::{GameMode, GameState};
    use crate::game::resources::{CurrentTurn, Players};
    use crate::game::systems::camera::CameraRotationState;
    use crate::game::view_mode::ViewMode;
    use bevy::state::app::StatesPlugin;

    /// The camera plugin inside the game's real frame configuration.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin).init_state::<GameState>();
        crate::core::schedule::configure(&mut app);
        app.add_plugins(CameraPlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ViewMode>()
            .init_resource::<crate::PersistentEguiCamera>()
            .init_resource::<Players>()
            .init_resource::<CurrentTurn>()
            .init_resource::<GameMode>()
            .init_resource::<CameraRotationState>();
        app.update();
        app
    }

    fn press_v(app: &mut App) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.press(KeyCode::KeyV);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    }

    #[test]
    fn v_does_nothing_on_the_menu() {
        let mut app = app();
        press_v(&mut app);
        assert_eq!(*app.world().resource::<ViewMode>(), ViewMode::Standard3D);
    }

    #[test]
    fn v_toggles_the_view_in_a_game() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        press_v(&mut app);
        assert_ne!(*app.world().resource::<ViewMode>(), ViewMode::Standard3D);
    }
}