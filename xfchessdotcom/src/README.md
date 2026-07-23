# web-solana/src

Source tree of the web frontend. App shell: [main.tsx](main.tsx) mounts
[App.tsx](App.tsx), which wires the wallet-adapter context, Privy provider, Chakra
theme, and `react-router-dom` v7 routes.

## Layout

| Path | Contents |
|------|----------|
| [pages/](pages/README.md) | One component per route |
| [components/](components/README.md) | Shared UI used across pages |
| [lib/](lib/README.md) | Backend API client, Anchor client, MagicBlock helpers |
| [hooks/](hooks/) | `useWalletUsdBalance.ts` — SOL→USD balance via price lookup |
| [privy/](privy/README.md) | Privy embedded-wallet provider + auth |
| [assets/](assets/) | Static images |

## Conventions

- Backend calls go through [lib/api/client.ts](lib/api/client.ts) (`VITE_BACKEND_URL`)
  — no raw `fetch` to the backend in pages.
- On-chain access goes through [lib/anchor_client.ts](lib/anchor_client.ts) /
  [lib/magicblock.ts](lib/magicblock.ts); never construct raw transactions.
- Wallet state comes from `useWallet()` (wallet-adapter) — the context wraps the
  whole app in [App.tsx](App.tsx).
