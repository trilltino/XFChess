use bevy::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

/// Initialize the Bevy chess game for WebAssembly
///
/// This function sets up the Bevy app with the provided canvas element.
/// The canvas will be used by Bevy for rendering.
#[wasm_bindgen]
pub fn init_bevy(canvas: HtmlCanvasElement) -> Result<(), JsValue> {
    // Set up panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Get canvas dimensions
    let width = canvas.width();
    let height = canvas.height();

    web_sys::console::log_1(
        &format!(
            "Initializing Bevy app for WASM with canvas {}x{}",
            width, height
        )
        .into(),
    );

    // Set the canvas ID - Bevy will automatically find and use it
    canvas.set_id("bevy");

    // Spawn the Bevy app in a separate task
    // This is necessary because .run() blocks, so we need to run it asynchronously
    wasm_bindgen_futures::spawn_local(async move {
        // Build and run the Bevy app
        // Note: This will need to import from xfchess crate to build the full app
        // For now, this is a placeholder structure
        let mut app = App::new();

        // Add default plugins with WASM-compatible configuration
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#bevy".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // For WASM, assets should be loaded via HTTP/HTTPS
                    // The path should be relative to the served directory
                    file_path: "assets".to_string(),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "warn,error".to_string(),
                    ..default()
                }),
        );

        // TODO: Add all the plugins and systems from the main app
        // This would include:
        // - CorePlugin
        // - EguiPlugin
        // - All game plugins
        // - All state plugins
        // etc.

        web_sys::console::log_1(&"Starting Bevy app...".into());

        // Run the app
        // Note: In WASM, this will run in the browser's event loop
        app.run();
    });

    Ok(())
}
