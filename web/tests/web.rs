//! WASM-specific tests
//!
//! These tests run in a browser environment using wasm-pack test.
//! Run with: cd web && wasm-pack test --headless --chrome

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Test that WASM module initializes without panicking
#[wasm_bindgen_test]
fn test_wasm_initializes() {
    // If we get here, WASM loaded successfully
    assert!(true, "WASM module loaded");
}

/// Test console logging works
#[wasm_bindgen_test]
fn test_console_logging() {
    console_log::init_with_level(log::Level::Debug).ok();
    log::info!("WASM test logging works!");
    assert!(true);
}

/// Test web_sys window access
#[wasm_bindgen_test]
fn test_window_exists() {
    let window = web_sys::window();
    assert!(window.is_some(), "Window should exist in browser context");
}

/// Test document access
#[wasm_bindgen_test]
fn test_document_exists() {
    let window = web_sys::window().expect("Window should exist");
    let document = window.document();
    assert!(document.is_some(), "Document should exist");
}

/// Test performance.now() is available
#[wasm_bindgen_test]
fn test_performance_timing() {
    let window = web_sys::window().expect("Window should exist");
    let performance = window.performance();
    assert!(performance.is_some(), "Performance API should be available");

    if let Some(perf) = performance {
        let now = perf.now();
        assert!(now >= 0.0, "performance.now() should return non-negative");
    }
}

/// Test web-time crate works (used instead of std::time::Instant)
#[wasm_bindgen_test]
fn test_web_time() {
    use web_time::Instant;

    let start = Instant::now();
    // Do some work
    let mut sum = 0u64;
    for i in 0..1000 {
        sum += i;
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_nanos() >= 0,
        "web_time should measure elapsed time"
    );
    assert!(sum > 0, "Work was done");
}

/// Test that getrandom works in WASM
#[wasm_bindgen_test]
fn test_random_generation() {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("getrandom should work in WASM");

    // Very unlikely to be all zeros
    assert!(
        buf.iter().any(|&b| b != 0),
        "Random bytes should not be all zeros"
    );
}

/// Test memory allocation works
#[wasm_bindgen_test]
fn test_memory_allocation() {
    // Allocate a reasonably large vector
    let v: Vec<u32> = (0..10_000).collect();
    assert_eq!(v.len(), 10_000);
    assert_eq!(v[9999], 9999);
}

/// Test string handling
#[wasm_bindgen_test]
fn test_string_operations() {
    let s = String::from("Hello, WASM!");
    assert_eq!(s.len(), 12);
    assert!(s.contains("WASM"));
}

/// Test async/await in WASM
#[wasm_bindgen_test]
async fn test_async_await() {
    // Simple async operation
    let result = async_helper().await;
    assert_eq!(result, 42);
}

async fn async_helper() -> i32 {
    42
}
