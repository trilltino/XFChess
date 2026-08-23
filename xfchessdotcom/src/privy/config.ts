/**
 * Privy configuration.
 *
 * Set `VITE_PRIVY_APP_ID` (from https://dashboard.privy.io) to enable social
 * login. When unset, the app runs exactly as before — Privy is a no-op, no
 * provider is mounted, no wallet is registered — so the build and runtime never
 * break on a missing app ID. This is also the kill switch: unset the var and
 * redeploy to remove social login entirely without a code change.
 *
 * See docs/plans/social-login-embedded-wallet-plan.md §14.
 */
export const PRIVY_APP_ID = (import.meta.env.VITE_PRIVY_APP_ID as string | undefined) || '';

export const PRIVY_ENABLED = PRIVY_APP_ID.length > 0;

/**
 * Name Privy gives its embedded wallet in the Wallet Standard registry. Used
 * both to find the wallet to register (see PrivyStandardBridge) and to identify
 * it once wallet-adapter has wrapped it in a StandardWalletAdapter (see
 * WalletSelectionModal).
 */
export const PRIVY_WALLET_NAME = 'Privy';
