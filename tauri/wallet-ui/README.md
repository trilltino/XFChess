# tauri/wallet-ui

React app for the desktop wrapper's wallet window: connect a wallet (extension or
Privy), report the pubkey/JWT to the Rust shell, and sign transactions the game
queues.

## Role in XFChess

The signing half of the wallet bridge described in [../README.md](../README.md):

```
Bevy game ──POST /pending──> tauri bridge (:7454) ──window event──> wallet-ui
wallet-ui signs ──POST /resolved──> bridge ──oneshot──> game continues
```

## Layout

| File | Contents |
|------|----------|
| [src/App.tsx](src/App.tsx) | The whole flow: wallet connect, pending-tx polling, signing UI, auth hand-off |
| [src/main.tsx](src/main.tsx) | Mount point |

## Run

```bash
npm install
npm run dev                          # standalone (dev server on :5174)
cargo tauri dev --features wallet    # inside the desktop shell (from tauri/)
```

## Invariants

- This window loads remote wallet scripts, so it must never hold shell capabilities
  (see [../capabilities/](../capabilities/)) — signing only.
- Tokens are handed to the shell via the bridge endpoints; don't persist them in
  browser storage here.
