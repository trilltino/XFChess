//! XFChess WASM — Bevy app compiled to WebAssembly for in-browser spectate/play.
//!
//! Entry points:
//! - `main()` — auto-called by `wasm_bindgen(start)`, builds the Bevy app
//!   targeting the `#xfchess` canvas.
//! - `load_game(id, mode)` — subscribes to a game's move stream via Braid-HTTP.
//! - `load_tournament(id)` — subscribes to standings/pairings.
//! - `sign_callback(cb)` — registers the JS wallet signing bridge.

use wasm_bindgen::prelude::*;

mod app;
mod braid_wasm;
mod board_mode;

/// Boot the Bevy app into the `#xfchess` canvas element.
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    app::run();
}

/// Subscribe to a game's move stream via Braid-HTTP.
#[wasm_bindgen]
pub fn load_game(game_id: u64, mode: &str) {
    crate::braid_wasm::subscribe_game(game_id, mode);
}

/// Subscribe to a tournament's standings + pairings.
#[wasm_bindgen]
pub fn load_tournament(tournament_id: u64) {
    crate::braid_wasm::subscribe_tournament(tournament_id);
}

/// Register a JS callback for wallet signing.
/// The callback receives `Uint8Array` (transaction bytes) and must return
/// `Promise<Uint8Array>` (signed transaction bytes).
#[wasm_bindgen]
pub fn sign_callback(cb: js_sys::Function) {
    crate::braid_wasm::set_sign_callback(cb);
}
