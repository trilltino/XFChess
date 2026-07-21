# web-solana/ — web frontend

React 19 + TypeScript + Vite web app: wallet-based sign-in, tournament registration
and play, spectating, KYC, and Solana Blinks integration. Talks to the backend API
(`VITE_BACKEND_URL`) and directly to Solana RPC via `@coral-xyz/anchor`.

## Role in XFChess

The browser counterpart to the native Bevy client: account/tournament management and
browser play, while the desktop app handles native play. It completes and signs the
partially-built transactions the backend returns. Ephemeral Rollup moves are submitted
by the backend, not the browser directly — see [MAGICBLOCK.md](../MAGICBLOCK.md) at the
repo root for the base-RPC-vs-Magic-Router routing the backend does on the web client's
behalf.

## Commands

```bash
npm install
npm run dev       # Vite dev server
npm run build     # tsc -b && vite build → dist/
npm run lint      # ESLint
npm run preview   # serve the production build locally
```

## Layout

| Path | Contents |
|------|----------|
| [src/pages/](src/pages/) | Routes: SignIn, Play, Tournaments/TournamentPlay, Spectate, FundWallet, Kyc, … |
| [src/components/](src/components/) | Shared UI: LoginModal, KycModal, MatchHistory, wallet selection |
| [src/lib/](src/lib/) | Backend API client ([api/](src/lib/api/)), Anchor client, MagicBlock helpers |
| [src/hooks/](src/hooks/) | e.g. `useWalletUsdBalance` |
| [src/privy/](src/privy/) | Privy embedded-wallet provider + auth button |

See [src/README.md](src/README.md) for module details.

## Example

```ts
// src/lib/api/client.ts — all backend calls share one base URL
const base =
  (import.meta.env.VITE_BACKEND_URL as string | undefined) || "http://127.0.0.1:8090";
```

## Invariants

- Regenerate and copy the `xfchess-game` Anchor IDL here (`anchor build`) whenever
  program instructions change.
- Build transactions through Anchor's `Program` class — never raw transactions.
- Every `VITE_*` variable is baked into the public bundle; secrets must stay
  server-side (see F3 in [deploy/docs/FRONTEND_REMEDIATION.md](../deploy/docs/FRONTEND_REMEDIATION.md)).
