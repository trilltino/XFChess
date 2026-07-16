# tauri/ — desktop wrapper

Tauri 2 desktop app (`xfchess-tauri`) that bridges browser-based Solana wallets to the
native Bevy game: it hosts the wallet UI, runs a localhost HTTP bridge the game and
website call for auth tokens and transaction signing, and embeds the tournament-admin
window.

## Role in XFChess

Browser wallet extensions (Phantom, Solflare) and Privy can't talk to a native game
process directly. This app sits between them:

```
Bevy game / website ──HTTP :7454──> tauri bridge ──window──> wallet-ui (extension / Privy signing)
tournament-admin window ──HTTP──> backend admin API (ADMIN_API_KEY)
xfchess:// deep link ──> game launch
```

The bridge holds the wallet pubkey, username, and backend JWT in shared state
([src/main.rs](src/main.rs)) and forwards signing requests to the wallet window via a
oneshot channel.

## Layout

| Path | Contents |
|------|----------|
| [src/](src/) | Rust shell: entry point, IPC commands, window management (see [src/README.md](src/README.md)) |
| [wallet-ui/](wallet-ui/) | React wallet window (connect, sign, balance) |
| [wallet-popup/](wallet-popup/) | Static HTML popup for browser-extension signing |
| [tournament-admin/](tournament-admin/) | React tournament administration app (see [tournament-admin/README.md](tournament-admin/README.md)) |
| [viz/](viz/) | Separate Tauri app: ER benchmark visualizer (see [viz/README.md](viz/README.md)) |
| [capabilities/](capabilities/) | Tauri permission scopes — shell access is restricted to the tournament-admin window ([capabilities/admin-shell.json](capabilities/admin-shell.json)) |
| [installer/](installer/), [macos/](macos/) | NSIS installer script and macOS packaging |

## The wallet bridge (`:7454`)

`main.rs` runs an Axum server on `XFCHESS_WALLET_PORT` (default 7454). Real routes:

```rust
.route("/pending", get(get_pending))          // game polls for tx to sign
.route("/resolved", post(post_resolved))      // wallet returns signed tx
.route("/wallet", post(post_wallet))          // wallet-ui reports pubkey
.route("/status", get(get_status))
.route("/token", get(get_token).post(post_token))  // backend JWT hand-off
.route("/api/auth/login", post(api_login))
```

CORS is restricted to local/tauri origins. Further hardening (per-launch bearer token,
signed updater, CSP) is tracked in [docs/TAURI_REMEDIATION.md](docs/TAURI_REMEDIATION.md).

## Build & run

```bash
cd wallet-ui && npm install && cd ../tournament-admin && npm install && cd ..
cargo tauri dev --features all        # dev mode
cargo tauri build --features all      # release bundle -> src-tauri/target/release/bundle/
cargo test                            # Rust unit tests (services::config, utils::logging)
```

Feature flags ([Cargo.toml](Cargo.toml)): `wallet`, `tournament-admin`, `dev`,
`production`, `all` (= dev + wallet + tournament-admin).

Environment: `XFCHESS_WALLET_PORT` (7454), `SIGNING_SERVICE_URL` /
`BACKEND_URL` (default `http://127.0.0.1:8090`), `ADMIN_API_KEY`, `RUST_LOG`.
