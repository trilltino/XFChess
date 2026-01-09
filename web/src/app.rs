use leptos::html::Canvas;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

#[component]
pub fn ChessApp() -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let initialized = RwSignal::new(false);
    let error_msg = RwSignal::new(String::new());

    // Initialize Bevy when component mounts and canvas is ready
    Effect::new(move |_| {
        if initialized.get() {
            return;
        }

        if let Some(canvas) = canvas_ref.get() {
            // Get HtmlCanvasElement from Leptos Canvas
            let canvas_element = canvas
                .dyn_ref::<HtmlCanvasElement>()
                .expect("Canvas element should be HtmlCanvasElement");

            // Set canvas ID for Bevy to find it
            // (bevy_wasm.rs will also set this, but setting it early ensures it's available)
            canvas_element.set_id("bevy");
            let canvas_id = "bevy".to_string();

            // Use request_animation_frame to ensure canvas is fully ready
            let window = web_sys::window().expect("no global `window` exists");
            let window_clone = window.clone();
            let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                web_sys::console::log_1(&"RAF callback started".into());

                // Get canvas element by ID from DOM
                let document = window_clone.document().expect("no document");
                let canvas_res = document
                    .get_element_by_id(&canvas_id)
                    .and_then(|el| el.dyn_into::<HtmlCanvasElement>().ok());

                if let Some(canvas) = canvas_res {
                    web_sys::console::log_1(
                        &"Canvas found, scheduling Bevy init on next tick...".into(),
                    );

                    // Use setTimeout to break the stack and avoid winit re-entrancy issues
                    let window_inner = web_sys::window().expect("no global window");

                    let timeout_cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                        web_sys::console::log_1(&"Timeout fired, initializing Bevy...".into());
                        if let Err(e) = crate::bevy_wasm::init_bevy(canvas) {
                            let msg = format!("Failed to initialize Bevy: {:?}", e);
                            web_sys::console::error_1(&msg.clone().into());
                            error_msg.set(msg);
                        } else {
                            web_sys::console::log_1(
                                &"Bevy initialized successfully, hiding loading screen".into(),
                            );
                            initialized.set(true);
                        }
                    });

                    // 100ms delay to be safe and let everything settle
                    let _ = window_inner.set_timeout_with_callback_and_timeout_and_arguments_0(
                        timeout_cb.unchecked_ref(),
                        100,
                    );
                } else {
                    let msg = "Canvas element NOT found in RAF callback".to_string();
                    web_sys::console::error_1(&msg.clone().into());
                    error_msg.set(msg);
                }
            }) as Box<dyn FnMut()>);

            window
                .request_animation_frame(closure.as_ref().unchecked_ref())
                .expect("Failed to request animation frame");

            closure.forget();
        }
    });

    // Styles for the premium UI
    let header_style = "height: 64px; background: rgba(13, 13, 25, 0.85); backdrop-filter: blur(12px); border-bottom: 1px solid rgba(255,255,255,0.08); display: flex; align-items: center; justify-content: space-between; padding: 0 32px; z-index: 20; box-shadow: 0 4px 20px rgba(0,0,0,0.4);";
    let title_style = "color: white; font-family: 'Outfit', sans-serif; font-size: 20px; font-weight: 600; letter-spacing: 0.5px; background: linear-gradient(90deg, #fff, #a0a0ff); -webkit-background-clip: text; -webkit-text-fill-color: transparent;";
    // let status_badge_style = "background: rgba(100, 255, 100, 0.1); border: 1px solid rgba(100, 255, 100, 0.2); color: #4ade80; font-family: 'JetBrains Mono', monospace; font-size: 11px; padding: 4px 8px; border-radius: 4px; display: flex; align-items: center; gap: 6px;";
    let container_style = "width: 100vw; height: 100vh; margin: 0; padding: 0; overflow: hidden; background: radial-gradient(circle at center, #1a1a2e 0%, #0a0a15 100%); display: flex; flex-direction: column;";

    view! {
        <div style=container_style>
            // Premium Header
            <header style=header_style>
                <div style="display: flex; align-items: center; gap: 16px;">
                    <h1 style=title_style>"XFCHESS"</h1>
                    <div style="width: 1px; height: 24px; background: rgba(255,255,255,0.1);"></div>
                    <span style="color: rgba(255,255,255,0.5); font-size: 13px; font-family: sans-serif; letter-spacing: 1px;">"WEB CLIENT"</span>
                </div>

                <div style="display: flex; align-items: center; gap: 24px;">
                    // Badge removed as requested
                </div>
            </header>

            // Game Container
            <div style="flex: 1; position: relative; width: 100%; height: 100%; overflow: hidden;">

                <Show when=move || !initialized.get()>
                    <div
                        style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; pointer-events: none; z-index: 5;"
                    >
                        <div class="spinner" style="margin: 0 auto 20px auto; border-color: rgba(255,255,255,0.1); border-top-color: #6c63ff;"></div>
                        <h2 style="color: white; font-family: 'Outfit', sans-serif; font-weight: 300; letter-spacing: 2px; margin-bottom: 8px;">"INITIALIZING NEURAL LINK"</h2>
                        <p style="color: rgba(255,255,255,0.4); font-size: 12px; font-family: monospace;">"Loading Assets..."</p>
                        <p style="color: #ff4444; font-size: 12px; font-family: monospace; margin-top: 10px;">{move || error_msg.get()}</p>
                    </div>
                </Show>

                <canvas
                    node_ref=canvas_ref
                    style="width: 100%; height: 100%; display: block; touch-action: none; outline: none;"
                    on:contextmenu=move |e| e.prevent_default()
                ></canvas>
            </div>
        </div>
    }
}
