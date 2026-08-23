/**
 * Privy configuration for the desktop wallet popup.
 *
 * Set `VITE_PRIVY_APP_ID` to enable social login. When unset the popup behaves
 * exactly as before — Phantom/Solflare only — so a build with no Privy app
 * configured is fully functional rather than broken.
 *
 * ## Allowed origins
 *
 * This page is served by the Tauri bridge at `http://localhost:<port>/wallet-ui/`,
 * where the port is 7454 nominally but scans up to 7464 when another instance
 * already holds it (see `bind_http_port` in tauri/src/main.rs). Privy matches
 * allowed origins EXACTLY, port included, with no wildcard on port — so every
 * one of those eleven origins has to be listed in the dashboard, or the popup
 * will fail for any user whose 7454 happens to be taken.
 *
 * `http://localhost` is a secure context in Chrome, so Privy's WebCrypto/TEE
 * paths work here despite the lack of TLS.
 *
 * See docs/plans/social-login-embedded-wallet-plan.md §7.1 and §10.
 */
export const PRIVY_APP_ID = (import.meta.env.VITE_PRIVY_APP_ID as string | undefined) || '';

export const PRIVY_ENABLED = PRIVY_APP_ID.length > 0;

/**
 * The Solana chain every Privy signing call runs against.
 *
 * Privy's `signTransaction`/`signAndSendTransaction` take an optional `chain`
 * and **default to `solana:mainnet`** when it is omitted. XFChess is a devnet
 * deployment (see `ensureDevnet`, which nudges extension wallets the same way,
 * and the devnet program ID in CLAUDE.md), so leaving it defaulted pointed
 * Privy at a chain this app has no RPC for — signing died with
 * "No RPC configuration found for chain solana:mainnet" before the user ever
 * saw an approval prompt.
 *
 * Every Privy signing call site must pass this. Changing deployment cluster is
 * a single edit here plus the two RPC URLs below.
 */
export const SOLANA_CHAIN = 'solana:devnet' as const;

/**
 * RPC endpoints handed to Privy for `SOLANA_CHAIN`.
 *
 * Privy needs its own RPC to simulate and display a transaction inside its
 * signing UI. It is NOT the broadcast path — a signed transaction still goes
 * back through the bridge to `/api/auth/broadcast-tx` — so the public endpoint
 * is adequate here. Override for a dedicated endpoint if rate limiting bites.
 */
export const SOLANA_RPC_URL =
  (import.meta.env.VITE_SOLANA_RPC_URL as string | undefined) || 'https://api.devnet.solana.com';

export const SOLANA_RPC_WS_URL =
  (import.meta.env.VITE_SOLANA_RPC_WS_URL as string | undefined) || 'wss://api.devnet.solana.com';
