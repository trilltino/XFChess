# Admin Panel Launch Scripts

The tournament admin panel is **desktop-only**: the UI is built to `dist/` and
rendered inside the Tauri `tournament-admin` window, served from the
loopback-only wallet bridge. It never runs as a standalone web/vite dev server.

## Launching

From the repo root:

```bash
just admin                          # opens the desktop admin window
scripts\start-tournament-admin.bat  # same, plus starts a local backend
```

Or from a running dev stack (`just dev`): right-click the tray icon →
**Tournament Admin**.

## After changing the UI

Rebuild the static bundle so the desktop window picks it up:

```bash
npm run build          # from tauri/tournament-admin
just build-admin-ui-force   # from the repo root
```

`launch-admin.bat` delegates to the desktop launcher; `launch-admin.js` is a
tombstone that explains the desktop-only flow.
