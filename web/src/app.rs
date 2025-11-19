use leptos::html::Canvas;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

#[component]
pub fn ChessApp() -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let initialized = create_rw_signal(false);

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

            // Set canvas dimensions based on window size
            let window = web_sys::window().expect("no global `window` exists");
            let width = window
                .inner_width()
                .ok()
                .and_then(|w| w.as_f64())
                .unwrap_or(800.0) as u32;
            let height = window
                .inner_height()
                .ok()
                .and_then(|h| h.as_f64())
                .unwrap_or(600.0) as u32;

            // Set explicit width and height attributes (required for Bevy)
            canvas_element.set_width(width);
            canvas_element.set_height(height);

            web_sys::console::log_1(
                &format!("Canvas initialized with size {}x{}", width, height).into(),
            );

            // Set canvas ID for Bevy to find it
            // (bevy_wasm.rs will also set this, but setting it early ensures it's available)
            canvas_element.set_id("bevy");
            let canvas_id = "bevy".to_string();

            // Use request_animation_frame to ensure canvas is fully ready
            let window_clone = window.clone();
            let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                // Get canvas element by ID from DOM
                let document = window_clone.document().expect("no document");
                let canvas = document
                    .get_element_by_id(&canvas_id)
                    .and_then(|el| el.dyn_into::<HtmlCanvasElement>().ok())
                    .expect("Canvas element not found");

                if let Err(e) = crate::bevy_wasm::init_bevy(canvas) {
                    web_sys::console::error_1(
                        &format!("Failed to initialize Bevy: {:?}", e).into(),
                    );
                } else {
                    initialized.set(true);
                }
            }) as Box<dyn FnMut()>);

            window
                .request_animation_frame(closure.as_ref().unchecked_ref())
                .expect("Failed to request animation frame");

            closure.forget();
        }
    });

    view! {
        <div style="width: 100vw; height: 100vh; margin: 0; padding: 0; overflow: hidden; background: #1a1a1a;">
            <canvas
                node_ref=canvas_ref
                style="width: 100%; height: 100%; display: block;"
            ></canvas>
        </div>
    }
}
