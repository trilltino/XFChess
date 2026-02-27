import { PublicKey } from '@solana/web3.js'
import { BN } from '@coral-xyz/anchor'

// Program ID from the deployed contract
export const PROGRAM_ID = new PublicKey('3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP')

// Magic Block program IDs
export const DELEGATION_PROGRAM_ID = new PublicKey('DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh')
export const MAGIC_BLOCK_PROGRAM_ID = new PublicKey('Magic11111111111111111111111111111111111111')

// Seed constants (must match the Rust program)
export const GAME_SEED = Buffer.from('game')
export const MOVE_LOG_SEED = Buffer.from('move_log')
export const WAGER_ESCROW_SEED = Buffer.from('escrow')
export const PROFILE_SEED = Buffer.from('profile')
export const SESSION_DELEGATION_SEED = Buffer.from('session_delegation')

// Delegation seeds for Magic Block ER
export const DELEGATION_RECORD_SEED = Buffer.from('delegation_record')
export const DELEGATION_METADATA_SEED = Buffer.from('delegation_metadata')
export const BUFFER_SEED = Buffer.from('buffer')

/**
 * Derive the Game PDA for a given game ID
 * @param gameId - The unique game ID (u64)
 * @returns [PublicKey, bump] - The PDA and its bump seed
 */
export function deriveGamePDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [GAME_SEED, gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

/**
 * Derive the MoveLog PDA for a given game ID
 * @param gameId - The unique game ID (u64)
 * @returns [PublicKey, bump] - The PDA and its bump seed
 */
export function deriveMoveLogPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [MOVE_LOG_SEED, gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

/**
 * Derive the Escrow PDA for a given game ID
 * @param gameId - The unique game ID (u64)
 * @returns [PublicKey, bump] - The PDA and its bump seed
 */
export function deriveEscrowPDA(gameId: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [WAGER_ESCROW_SEED, gameId.toArrayLike(Buffer, 'le', 8)],
        PROGRAM_ID
    )
}

/**
 * Derive the PlayerProfile PDA for a given player public key
 * @param player - The player's public key
 * @returns [PublicKey, bump] - The PDA and its bump seed
 */
export function deriveProfilePDA(player: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [PROFILE_SEED, player.toBuffer()],
        PROGRAM_ID
    )
}

/**
 * Derive the SessionDelegation PDA for a given game and player
 * @param gameId - The unique game ID (u64)
 * @param player - The player's public key
 * @returns [PublicKey, bump] - The PDA and its bump seed
 */
export function deriveSessionDelegationPDA(gameId: BN, player: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [SESSION_DELEGATION_SEED, gameId.toArrayLike(Buffer, 'le', 8), player.toBuffer()],
        PROGRAM_ID
    )
}

/**
 * Generate a unique game ID based on current timestamp
 * @returns BN - A unique game ID as a BigNumber
 */
export function generateGameId(): BN {
    return new BN(Date.now())
}

/**
 * Convert SOL to lamports
 * @param sol - Amount in SOL
 * @returns BN - Amount in lamports
 */
export function solToLamports(sol: number): BN {
    return new BN(Math.round(sol * 1e9))
}

/**
 * Convert lamports to SOL
 * @param lamports - Amount in lamports
 * @returns number - Amount in SOL
 */
export function lamportsToSol(lamports: BN | number): number {
    const lamportsNum = typeof lamports === 'number' ? lamports : lamports.toNumber()
    return lamportsNum / 1e9
}

/**
 * Derive the Delegation Record PDA for a given game PDA (Magic Block ER)
 * @param gamePDA - The game PDA being delegated
 * @returns [PublicKey, bump] - The delegation record PDA and its bump seed
 */
export function deriveDelegationPDA(gamePDA: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [DELEGATION_RECORD_SEED, gamePDA.toBuffer()],
        DELEGATION_PROGRAM_ID
    )
}

/**
 * Derive the Delegation Metadata PDA for a given game PDA (Magic Block ER)
 * @param gamePDA - The game PDA being delegated
 * @returns [PublicKey, bump] - The delegation metadata PDA and its bump seed
 */
export function deriveDelegationMetadataPDA(gamePDA: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [DELEGATION_METADATA_SEED, gamePDA.toBuffer()],
        DELEGATION_PROGRAM_ID
    )
}

/**
 * Derive the Buffer PDA for a given game PDA (Magic Block ER)
 * @param gamePDA - The game PDA being delegated
 * @returns [PublicKey, bump] - The buffer PDA and its bump seed
 */
export function deriveBufferPDA(gamePDA: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
        [BUFFER_SEED, gamePDA.toBuffer()],
        DELEGATION_PROGRAM_ID
    )
}
