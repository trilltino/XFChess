// Privy configuration. Set VITE_PRIVY_APP_ID (from https://dashboard.privy.io)
// to enable Privy auth. When unset, the app runs exactly as before (Privy is a
// no-op), so the build/runtime never break on a missing app ID.
export const PRIVY_APP_ID = (import.meta.env.VITE_PRIVY_APP_ID as string | undefined) || '';
export const PRIVY_ENABLED = PRIVY_APP_ID.length > 0;
