# Desktop shell

`main.rs` configures Tauri and starts the wallet bridge. Wallet code lives in
`wallet/`: HTTP routes, TCP framing, request completion, backend proxying, static
assets, popup control, and game-process lifetime each have their own module.

On Windows, the launcher supplies `XFCHESS_GAME_PID`; development games can
register their PID through `/status`. The bridge retains a
process handle and cancels signing, closes its wallet popup, removes its port
announcements, and exits when that game ends. The Chromium popup is owned by the
game window and identified by its port and random session ID. Browser processes
are shared with ordinary tabs, so cleanup closes the wallet window.

The game submits signing requests over TCP; the browser receives them over SSE.
Resolution and cancellation must match the active request ID. Signing and wallet
state endpoints require the popup session ID; browser origins are checked.

`services/` contains IPC, configuration, authentication, and notifications.
`windows/tournament_admin.rs` contains the optional admin window, SSH tunnel, and
OS credential store. Admin routes and capabilities require `tournament-admin`;
wallet pages never receive shell capabilities.

Run `cargo test -p xfchess-tauri --bin xfchess-tauri` for bridge and process tests.
Packages build with `custom-protocol` to serve the embedded wallet assets.
