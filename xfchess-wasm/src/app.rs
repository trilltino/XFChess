//! Bevy app builder for the WASM target.

use bevy::prelude::*;

/// Build and run the Bevy app targeting the `#xfchess` canvas.
pub fn run() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            canvas: Some("#xfchess".to_string()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }));

    // Minimal chess board rendering setup
    app.insert_resource(crate::board_mode::BoardRenderMode::ThreeD);
    app.add_systems(Startup, setup_camera);
    app.add_systems(Update, crate::board_mode::toggle_board_mode);

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera3d::default());
}
