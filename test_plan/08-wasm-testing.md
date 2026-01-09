# WASM/Web Testing Guide

Testing WebAssembly builds requires special considerations.

## Challenges

1. **No std::time::Instant**: Use `web-time` crate
2. **No Threads**: Single-threaded execution
3. **Browser APIs**: Need JS interop for some features
4. **Build Time**: WASM compilation is slower

## Build Verification

### Basic WASM Build Test

```bash
# Verify WASM builds successfully
cargo build --target wasm32-unknown-unknown -p xfchess

# Build with release optimizations
cargo build --target wasm32-unknown-unknown -p xfchess --release
```

### Automated Build Check

```rust
// build.rs or CI script
#[test]
#[ignore] // Run manually or in CI
fn test_wasm_builds() {
    let status = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown"])
        .status()
        .expect("Failed to run cargo");
    
    assert!(status.success(), "WASM build failed");
}
```

## WASM-Specific Code Testing

### Conditional Compilation

```rust
// src/utils/time.rs
#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    web_time::Instant::now().elapsed().as_secs_f64()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    std::time::Instant::now().elapsed().as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_returns_positive() {
        let t = now();
        assert!(t >= 0.0);
    }
}
```

### Testing Platform-Specific Branches

```rust
#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_native_only_feature() {
        // This test only runs on native
    }
}
```

## wasm-pack Testing

For true browser testing, use `wasm-pack`:

```bash
# Install wasm-pack
cargo install wasm-pack

# Run tests in headless browser
wasm-pack test --headless --chrome
```

### wasm-pack Test Setup

```rust
// web/tests/web.rs
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_basic_wasm() {
    assert_eq!(1 + 1, 2);
}

#[wasm_bindgen_test]
async fn test_async_wasm() {
    // Test async code in browser
    let result = async_function().await;
    assert!(result.is_ok());
}
```

## Browser Automation Testing

For full E2E browser testing:

### Playwright/Puppeteer Setup

```javascript
// tests/e2e/game.spec.js
const { test, expect } = require('@playwright/test');

test('game loads successfully', async ({ page }) => {
  await page.goto('http://localhost:8080');
  
  // Wait for WASM to load
  await page.waitForSelector('#bevy-canvas', { timeout: 30000 });
  
  // Verify no console errors
  const errors = [];
  page.on('console', msg => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  
  expect(errors).toHaveLength(0);
});

test('main menu renders', async ({ page }) => {
  await page.goto('http://localhost:8080');
  await page.waitForTimeout(5000); // Wait for render
  
  // Take screenshot for visual comparison
  await expect(page).toHaveScreenshot('main-menu.png');
});
```

## Asset Loading Tests

```rust
#[test]
fn test_assets_exist_for_wasm() {
    let required_assets = [
        "assets/models/pieces/pawn.obj",
        "assets/textures/board.png",
        "assets/game_sounds/move_piece.mp3",
    ];
    
    for asset in required_assets {
        assert!(
            std::path::Path::new(asset).exists(),
            "Missing asset for WASM: {}",
            asset
        );
    }
}
```

## Storage Testing

```rust
// src/storage.rs
#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

pub fn save_settings(settings: &GameSettings) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        LocalStorage::set("settings", settings)
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native file storage
        std::fs::write("settings.json", serde_json::to_string(settings).unwrap())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_save_settings_native() {
        let settings = GameSettings::default();
        let result = save_settings(&settings);
        assert!(result.is_ok());
    }
}
```

## CI WASM Testing

```yaml
# .github/workflows/wasm.yml
name: WASM Tests

on: [push, pull_request]

jobs:
  wasm-build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      
      - name: Build WASM
        run: cargo build --target wasm32-unknown-unknown
      
      - name: Install wasm-pack
        run: cargo install wasm-pack
      
      - name: Run WASM tests
        run: wasm-pack test --headless --chrome web/
```

## Running WASM Tests

```bash
# Native tests (default)
cargo test

# WASM build verification
cargo build --target wasm32-unknown-unknown

# wasm-pack browser tests
cd web && wasm-pack test --headless --chrome

# Full browser E2E
npx playwright test
```
