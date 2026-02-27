import { useState, useCallback, useEffect } from 'react'
import { useConnection, useWallet } from '@solana/wallet-adapter-react'
import { BN } from '@coral-xyz/anchor'
import type { PublicKey } from '@solana/web3.js'
import {
    deriveGamePDA,
    deriveDelegationPDA,
    deriveDelegationMetadataPDA,
    deriveBufferPDA,
    PROGRAM_ID,
    DELEGATION_PROGRAM_ID,
    MAGIC_BLOCK_PROGRAM_ID,
} from '../utils/pda'
import { useGameProgram } from './useGameProgram'

type DelegationStatus = 'not_delegated' | 'delegating' | 'delegated' | 'undelegating' | 'error'

interface UseMagicBlockReturn {
    // Status
    status: DelegationStatus
    isDelegated: boolean
    isLoading: boolean
    error: string | null

    // Actions
    delegateGame: (gameId: string | BN, validUntil?: number) => Promise<void>
    undelegateGame: (gameId: string | BN) => Promise<void>
    checkStatus: (gameId: string | BN) => Promise<boolean>
    clearError: () => void
}

/**
 * Hook for Magic Block Ephemeral Rollups operations
 * Manages delegation status and provides actions for delegating/undelegating games
 * 
 * @example
 * ```tsx
 * function GameComponent({ gameId }: { gameId: string }) {
 *   const { status, delegateGame, undelegateGame, isLoading } = useMagicBlock()
 *   
 *   return (
 *     <div>
 *       <button onClick={() => delegateGame(gameId)} disabled={isLoading}>
 *         Delegate to ER
 *       </button>
 *       <button onClick={() => undelegateGame(gameId)} disabled={isLoading}>
 *         Undelegate
 *       </button>
 *       <span>Status: {status}</span>
 *     </div>
 *   )
 * }
 * ```
 */
export function useMagicBlock(): UseMagicBlockReturn {
    const { connection } = useConnection()
    const wallet = useWallet()
    const {
        delegateGame: programDelegateGame,
        undelegateGame: programUndelegateGame,
        checkDelegationStatus,
        isDelegated: programIsDelegated,
        isDelegating,
        delegationError,
        clearDelegationError,
    } = useGameProgram()

    const [status, setStatus] = useState<DelegationStatus>('not_delegated')
    const [error, setError] = useState<string | null>(null)

    // Sync status with program state
    useEffect(() => {
        if (isDelegating) {
            setStatus(prev => prev === 'delegated' ? 'undelegating' : 'delegating')
        } else if (programIsDelegated) {
            setStatus('delegated')
        } else if (delegationError) {
            setStatus('error')
            setError(delegationError)
        } else {
            setStatus('not_delegated')
        }
    }, [isDelegating, programIsDelegated, delegationError])

    /**
     * Convert gameId to BN if it's a string
     */
    const toBN = useCallback((gameId: string | BN): BN => {
        if (typeof gameId === 'string') {
            return new BN(gameId)
        }
        return gameId
    }, [])

    /**
     * Delegate a game to Magic Block Ephemeral Rollups
     * @param gameId - The game ID to delegate (string or BN)
     * @param validUntil - Unix timestamp when delegation expires (defaults to 24 hours from now)
     */
    const delegateGame = useCallback(async (
        gameId: string | BN,
        validUntil?: number
    ): Promise<void> => {
        if (!wallet.connected || !wallet.publicKey) {
            setError('Wallet not connected')
            setStatus('error')
            throw new Error('Wallet not connected')
        }

        setStatus('delegating')
        setError(null)
        clearDelegationError()

        try {
            const gameIdBN = toBN(gameId)
            const validUntilBN = validUntil
                ? new BN(validUntil)
                : new BN(Math.floor(Date.now() / 1000) + 86400) // 24 hours default

            await programDelegateGame(gameIdBN, validUntilBN)
            setStatus('delegated')
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Failed to delegate game'
            setError(errorMessage)
            setStatus('error')
            throw err
        }
    }, [wallet.connected, wallet.publicKey, programDelegateGame, toBN, clearDelegationError])

    /**
     * Undelegate a game from Magic Block Ephemeral Rollups
     * @param gameId - The game ID to undelegate (string or BN)
     */
    const undelegateGame = useCallback(async (
        gameId: string | BN
    ): Promise<void> => {
        if (!wallet.connected || !wallet.publicKey) {
            setError('Wallet not connected')
            setStatus('error')
            throw new Error('Wallet not connected')
        }

        setStatus('undelegating')
        setError(null)
        clearDelegationError()

        try {
            const gameIdBN = toBN(gameId)
            await programUndelegateGame(gameIdBN)
            setStatus('not_delegated')
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Failed to undelegate game'
            setError(errorMessage)
            setStatus('error')
            throw err
        }
    }, [wallet.connected, wallet.publicKey, programUndelegateGame, toBN, clearDelegationError])

    /**
     * Check the delegation status of a game
     * @param gameId - The game ID to check (string or BN)
     * @returns Whether the game is currently delegated
     */
    const checkStatus = useCallback(async (
        gameId: string | BN
    ): Promise<boolean> => {
        try {
            const gameIdBN = toBN(gameId)
            const isGameDelegated = await checkDelegationStatus(gameIdBN)
            setStatus(isGameDelegated ? 'delegated' : 'not_delegated')
            return isGameDelegated
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Failed to check delegation status'
            setError(errorMessage)
            setStatus('error')
            return false
        }
    }, [checkDelegationStatus, toBN])

    /**
     * Clear any error state
     */
    const clearError = useCallback(() => {
        setError(null)
        clearDelegationError()
        if (status === 'error') {
            setStatus('not_delegated')
        }
    }, [status, clearDelegationError])

    return {
        status,
        isDelegated: status === 'delegated',
        isLoading: status === 'delegating' || status === 'undelegating',
        error,
        delegateGame,
        undelegateGame,
        checkStatus,
        clearError,
    }
}

export type { DelegationStatus, UseMagicBlockReturn }
