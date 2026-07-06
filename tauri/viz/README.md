# XFChess Network Visualiser

Standalone **Tauri 2** desktop app that benchmarks your Triton RPC against public
devnet and draws the XFChess network topology — built for screen-recordable demos.

Self-contained: **excluded from the root Cargo workspace** (see `exclude` in the
repo-root `Cargo.toml`), so it builds and runs independently of the main app.

## Architecture

- **Frontend** (`src/`) — React 19 + Vite + ECharts. Charts + force-graph topology.
- **Backend** (`src-tauri/`) — Rust. The RPC load benchmark runs **natively** (reqwest),
  so there's **no browser CORS** and the Triton token never leaves the native process.
  Results stream to the webview via the `bench-level` / `bench-done` events.

```
React  --invoke('run_read_load')-->  Rust (reqwest ramp)
React  <--emit('bench-level')-------  Rust (per-level result)
```

## Run

```bash
cd tauri/viz
npm install
npm run tauri dev     # opens the desktop window
```

In the window: paste your Triton URL (`https://…rpcpool.com/<token>`), leave the
baseline as public devnet, hit **Run benchmark**. Watch throughput, the 429 (throttle)
chart, latency lines, and the topology graph (the backend↔Triton edge shows live p50).

## Build a distributable

```bash
cd tauri/viz
npm run tauri build   # installers in src-tauri/target/release/bundle/
```

## Notes

- The token is entered at runtime and redacted in the UI — safe to screen-record.
- Concurrency levels and requests/level are configurable in the toolbar.
- Topology nodes/edges live in `src/lib/topology.ts`; the Rust probe in
  `src-tauri/src/main.rs`.
