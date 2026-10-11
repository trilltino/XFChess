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
| [src/components/](src/components/) | One panel per admin area: Dashboard, tournaments/ (on-chain list/create/detail, plus OfflineTournamentAdmin + OfflineEventDetail for off-chain events), MatchManagement, PlayerList, Treasury, KycStatus, Puzzles, GameExplorer, PgnViewer, HetznerSsh, DeploymentManager, Settings, TokenAuth |
| [src/services/api.ts](src/services/api.ts) | Backend admin API client (attaches the API key, held in memory only) |
| [src/services/credentials.ts](src/services/credentials.ts) | Admin key in the OS credential store (Rust commands); migrates old localStorage keys once |
| [src/services/lifecycle.ts](src/services/lifecycle.ts) | `nextStep()` — the single next action for a tournament (fund, gather players, start, payout) |
| [src/components/tournaments/detail/](src/components/tournaments/detail/) | Tournament detail split by concern: data hook, lifecycle panel, Overview/Players/Matches tabs |
| [src/config/environments.ts](src/config/environments.ts) | LOCAL/PRODUCTION; VPS host + SSH key from `.env` (see `.env.example`) or the login screen/Settings |
| [src/hooks/useAuth.tsx](src/hooks/useAuth.tsx) | Admin token auth state |
| [src/types/tournament.ts](src/types/tournament.ts) | Shared tournament types |
| [scripts/](scripts/README.md) | Helper scripts |

## Run

The admin panel is desktop-only — it renders in the Tauri `tournament-admin`
window, served from the loopback-only wallet bridge. It never runs as a
standalone web/vite dev server.

```bash
just admin  # from the repo root: starts a local backend and opens the desktop window
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

- The admin key never touches webview storage: it lives in the OS credential store via
  `admin_token_*` commands, and the window can only navigate within `/tournament-admin/`.
- Tournament ids are assigned by the backend; the panel never invents one.
- Every request goes through [src/services/api.ts](src/services/api.ts) so the admin
  key is attached in one place.
- Deployment actions only surface instructions for
  [ops/scripts/deploy.ps1](../../ops/scripts/deploy.ps1) — the app must not
  execute deploys itself. See [operations](../../ops/README.md).
- Dashboard host resources come from authenticated `/admin/metrics/summary`,
  through the same environment/SSH tunnel as game administration. Missing samples
  show as unavailable; no public Prometheus URL or browser CORS workaround is used.
- The Deployment page displays the connected backend commit and configured
  release/rollback commands. Monitoring uses a separate `deploy` SSH forward;
  the native admin tunnel permits only `tunnel` to loopback backend port 8090.
