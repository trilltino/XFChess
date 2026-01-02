#!/bin/bash
# Build script for compiling XFChess to WebAssembly

set -e

echo "Building XFChess for WebAssembly..."

# Check if wasm32 target is installed
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Check if wasm-bindgen-cli is installed
if ! command -v wasm-bindgen &> /dev/null; then
    echo "Installing wasm-bindgen-cli..."
    cargo install wasm-bindgen-cli
fi

# Set RUSTFLAGS for WASM
export RUSTFLAGS="--cfg getrandom_backend=\"wasm_js\""

# Build the web package for WASM
echo "Building xfchess-web for wasm32-unknown-unknown..."
cargo build --package xfchess-web --target wasm32-unknown-unknown --release

# Generate JavaScript bindings
echo "Generating JavaScript bindings..."
wasm-bindgen \
    --out-dir web/pkg \
    --out-name xfchess_web \
    --target web \
    target/wasm32-unknown-unknown/release/xfchess_web.wasm

echo "Build complete! Output files are in web/pkg/"
echo "To serve the application, use:"
echo "  cd web && trunk serve"
echo "or"
echo "  cd web && python3 -m http.server"

