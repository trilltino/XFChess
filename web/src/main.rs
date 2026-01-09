use leptos::prelude::*;
use xfchess_web::app::ChessApp;

fn main() {
    // Initialize logging

    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    // Mount the Leptos app to the body
    mount_to_body(ChessApp);
}
