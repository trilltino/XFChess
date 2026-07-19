# tauri/src

Rust shell of the desktop wrapper: entry point, wallet HTTP bridge, IPC commands, and
window management. App-level overview: [../README.md](../README.md).

## Layout

| Path | Contents |
|------|----------|
| [main.rs](main.rs) | Entry point: `xfchess://` deep link, shared state (`WalletPubkey`, `WalletJwt`, `PendingTx`), and the Axum wallet bridge on `:7454` (no tray icon — windows open via the bridge API or `XFCHESS_OPEN_ADMIN=1`) |
| [services/ipc.rs](services/ipc.rs) | All `#[tauri::command]` handlers (windows, notifications, clipboard, URL opening) |
| [services/auth.rs](services/auth.rs) | Auth state management |
| [services/config.rs](services/config.rs) | Env-based configuration (tests: [services/config_tests.rs](services/config_tests.rs)) |
| [services/notification_poller.rs](services/notification_poller.rs) | Polls the backend for notifications → OS toast notifications (no tray icon) |
| [types/](types/) | Serde types for auth, config, and IPC payloads |
| [utils/](utils/) | [logging.rs](utils/logging.rs) (+ tests) |
| [windows/](windows/) | Window builders: [tournament_admin.rs](windows/tournament_admin.rs) (the wallet UI is a real Chrome `--app` popup opened via `open_in_browser` in main.rs, not a Tauri-managed window) |
| [error.rs](error.rs) | Unified error type |

## Example

```rust
// main.rs — a game signing request flows through shared state:
type PendingTxInner = Option<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, String>>)>;
// game POSTs tx → /pending queues it → wallet window signs → /resolved completes the oneshot
```

## Invariants

- IPC commands must not `.unwrap()` on frontend-supplied input (frontend-triggerable
  panic — see T7 in [../docs/TAURI_REMEDIATION.md](../docs/TAURI_REMEDIATION.md)).
- `open_url` allowlists schemes (`http/https/mailto/xfchess`); keep it that way.
- Shell capabilities are scoped to the tournament-admin window only
  ([../capabilities/](../capabilities/)) — never re-grant them to the wallet window.
