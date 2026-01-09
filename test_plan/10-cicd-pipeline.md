# CI/CD Testing Pipeline Guide

Automate testing with GitHub Actions.

## Workflow Structure

```
.github/workflows/
├── ci.yml           # Main CI pipeline
├── wasm.yml         # WASM-specific tests
└── release.yml      # Release builds
```

## Main CI Pipeline

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Check
        run: cargo check --workspace --all-features

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - name: Format
        run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Clippy
        run: cargo clippy --workspace --all-features -- -D warnings

  test:
    name: Test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install dependencies (Linux)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libasound2-dev libudev-dev
      - name: Test
        run: cargo test --workspace

  test-chess-engine:
    name: Chess Engine Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Test chess_engine
        run: cargo test -p chess_engine --all-features

  test-backend:
    name: Backend Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Test backend
        run: cargo test -p backend

  wasm-build:
    name: WASM Build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - name: Build WASM
        run: cargo build --target wasm32-unknown-unknown -p xfchess

  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install dependencies
        run: sudo apt-get install -y libasound2-dev libudev-dev
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      - name: Generate coverage
        run: cargo tarpaulin --workspace --out Xml
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: cobertura.xml
```

## WASM-Specific Pipeline

```yaml
# .github/workflows/wasm.yml
name: WASM

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  wasm-test:
    name: WASM Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      
      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
      
      - name: Install Chrome
        uses: browser-actions/setup-chrome@latest
      
      - name: WASM Tests
        run: |
          cd web
          wasm-pack test --headless --chrome
```

## Release Pipeline

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: wasm32-unknown-unknown
            os: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Install dependencies (Linux)
        if: matrix.os == 'ubuntu-latest' && matrix.target != 'wasm32-unknown-unknown'
        run: sudo apt-get install -y libasound2-dev libudev-dev
      
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: xfchess-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/xfchess*
```

## Local CI Simulation

```bash
#!/bin/bash
# scripts/ci-local.sh

set -e

echo "=== Format Check ==="
cargo fmt --all -- --check

echo "=== Clippy ==="
cargo clippy --workspace --all-features -- -D warnings

echo "=== Tests ==="
cargo test --workspace

echo "=== Chess Engine Tests ==="
cargo test -p chess_engine --all-features

echo "=== Backend Tests ==="
cargo test -p backend

echo "=== WASM Build ==="
cargo build --target wasm32-unknown-unknown

echo "=== All Checks Passed ==="
```

## Pre-commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: cargo fmt
        entry: cargo fmt --all -- --check
        language: system
        pass_filenames: false
      
      - id: cargo-check
        name: cargo check
        entry: cargo check --workspace
        language: system
        pass_filenames: false
      
      - id: cargo-test
        name: cargo test
        entry: cargo test --workspace --lib
        language: system
        pass_filenames: false
```

## Test Reporting

### JUnit Format for CI

```bash
# Install junit reporter
cargo install cargo2junit

# Run tests with junit output
cargo test --workspace -- -Z unstable-options --format json | cargo2junit > results.xml
```

### GitHub Actions Summary

```yaml
- name: Test with output
  run: |
    cargo test --workspace 2>&1 | tee test-output.txt
    echo "## Test Results" >> $GITHUB_STEP_SUMMARY
    echo '```' >> $GITHUB_STEP_SUMMARY
    tail -20 test-output.txt >> $GITHUB_STEP_SUMMARY
    echo '```' >> $GITHUB_STEP_SUMMARY
```

## Badge Setup

Add to README.md:

```markdown
![CI](https://github.com/trilltino/XFChess/actions/workflows/ci.yml/badge.svg)
![Coverage](https://codecov.io/gh/trilltino/XFChess/branch/main/graph/badge.svg)
```
