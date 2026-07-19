# tauri/tournament-admin

React admin app rendered in the desktop wrapper's tournament-admin window (1200×800,
gated behind `ADMIN_API_KEY`). Operates the live platform: tournaments, players,
treasury, KYC review, puzzles, and deployment status.

## Role in XFChess

Admin counterpart to the public clients — it drives the backend's `require_api_key`
routes ([backend/src/signing/routes/admin.rs](../../backend/src/signing/routes/admin.rs))
and, via the Tauri shell capability, an SSH quick-action to the production VPS
(scoped in [../capabilities/admin-shell.json](../capabilities/admin-shell.json)).

## Layout

| Path | Contents |
|------|----------|
| [src/components/](src/components/) | One panel per admin area: Dashboard, tournaments/ (list/create/detail), MatchManagement, PlayerList, Treasury, KycStatus, Puzzles, GameExplorer, PgnViewer, HetznerSsh, DeploymentManager, Settings, TokenAuth |
| [src/services/api.ts](src/services/api.ts) | Backend admin API client (attaches the API key) |
| [src/hooks/useAuth.tsx](src/hooks/useAuth.tsx) | Admin token auth state |
| [src/types/tournament.ts](src/types/tournament.ts) | Shared tournament types |
| [scripts/](scripts/README.md) | Helper scripts |

## Run

The admin panel is desktop-only — it renders in the Tauri `tournament-admin`
window, served from the loopback-only wallet bridge. It never runs as a
standalone web/vite dev server.

```bash
just admin                          # from the repo root: opens the desktop window
scripts\start-tournament-admin.bat  # same, plus starts a local backend
```

From a running dev stack (`just dev`): tray icon → **Tournament Admin**.

After changing the UI, rebuild the static bundle:

```bash
npm run build               # here, or:
just build-admin-ui-force   # from the repo root
```

## Invariants

- Desktop-only: the UI is served from the built `dist/` by the Tauri shell's
  loopback bridge — never add a vite dev server or expose it on a network port.

- Every request goes through [src/services/api.ts](src/services/api.ts) so the admin
  key is attached in one place.
- Deployment actions only surface instructions for
  [deploy/scripts/deploy.ps1](../../deploy/scripts/deploy.ps1) — the app must not
  execute deploys itself (T9 in [../docs/TAURI_REMEDIATION.md](../docs/TAURI_REMEDIATION.md)).
