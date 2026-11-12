/**
 * VITE_PRIVY_APP_ID enables social login. Register each localhost origin on
 * ports 7454-7464 in Privy: allowed origins include the exact port.
 */
export const PRIVY_APP_ID = (import.meta.env.VITE_PRIVY_APP_ID as string | undefined) || '';

export const PRIVY_ENABLED = PRIVY_APP_ID.length > 0;

/** Pass SOLANA_CHAIN to every Privy signing call; omitting it defaults to mainnet. */
export const SOLANA_CHAIN = 'solana:devnet' as const;

/**
 * Privy RPC simulates and displays transactions. Broadcasting still goes
 * through the bridge; override simulation endpoints if rate-limited.
 */
export const SOLANA_RPC_URL =
  (import.meta.env.VITE_SOLANA_RPC_URL as string | undefined) || 'https://api.devnet.solana.com';

export const SOLANA_RPC_WS_URL =
  (import.meta.env.VITE_SOLANA_RPC_WS_URL as string | undefined) || 'wss://api.devnet.solana.com';
