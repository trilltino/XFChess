//! Braid-HTTP subscription via `web_sys::fetch` for WASM builds.
//!
//! Uses the Braid 209 Subscribe protocol: sends `GET` with
//! `Subscribe: keep-alive` header, reads the `ReadableStream` body,
//! and yields `Update`s into a channel consumed by Bevy systems.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{js_sys, RequestInit, RequestMode, Response};
use std::cell::RefCell;

thread_local! {
    static SIGN_CALLBACK: RefCell<Option<js_sys::Function>> = RefCell::new(None);
}

/// Register a JS callback for wallet signing.
pub fn set_sign_callback(cb: js_sys::Function) {
    SIGN_CALLBACK.with(|cell| {
        *cell.borrow_mut() = Some(cb);
    });
}

/// Subscribe to a game's move stream.
pub fn subscribe_game(game_id: u64, _mode: &str) {
    let url = format!("/braid/game/{}/moves", game_id);
    spawn_local(async move {
        if let Err(e) = fetch_braid_stream(&url).await {
            tracing::error!("[braid-wasm] game subscribe failed: {:?}", e);
        }
    });
}

/// Subscribe to a tournament's standings + pairings.
pub fn subscribe_tournament(tournament_id: u64) {
    let url = format!("/braid/tournament/{}/standings", tournament_id);
    spawn_local(async move {
        if let Err(e) = fetch_braid_stream(&url).await {
            tracing::error!("[braid-wasm] tournament subscribe failed: {:?}", e);
        }
    });
}

/// Fetch a Braid-HTTP 209 Subscribe stream.
///
/// In a full implementation this would parse the multipart body
/// (Version:/Content-Length:/body framing) and dispatch updates
/// into a Bevy channel. For now we log the connection status.
async fn fetch_braid_stream(url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::SameOrigin);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)?;
    let headers = request.headers();
    headers.set("Subscribe", "keep-alive")?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    let status = resp.status();
    if status != 209 {
        tracing::warn!("[braid-wasm] unexpected status {} for {}", status, url);
    } else {
        tracing::info!("[braid-wasm] subscribed to {}", url);
    }

    // TODO: parse ReadableStream body for multipart updates
    Ok(())
}

/// Helper to convert a JS Promise into a Rust Future.
type JsFuture = wasm_bindgen_futures::JsFuture;
