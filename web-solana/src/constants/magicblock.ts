// MagicBlock Ephemeral Rollup Configuration
// These values are for hackathon development on devnet

export const MAGICBLOCK_CONFIG = {
    // The MagicBlock Ephemeral Rollup endpoint (EU region)
    RPC_URL: 'https://devnet-eu.magicblock.app',

    // Devnet validator ID for EU region
    VALIDATOR_ID: 'MEUGGrYPxKk17hCr7wpT6s8dtNokZj5U2L57vjYMS8e',

    // XFChess Game Program ID (deployed on devnet)
    PROGRAM_ID: '3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP',

    // Connection timeout in milliseconds
    TIMEOUT: 30000,

    // Commitment level for transactions
    COMMITMENT: 'confirmed' as const,
} as const;

// Session expiry time (2 hours)
export const SESSION_EXPIRY_MS = 2 * 60 * 60 * 1000;
