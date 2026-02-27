import { useState, useCallback } from 'react'
import { useWallet } from '@solana/wallet-adapter-react'
import { PublicKey } from '@solana/web3.js'
import { BN } from '@coral-xyz/anchor'
import {
    generateSessionKey,
    generateNodeId,
    launchGameClient,
    writeLaunchParamsFile,
    storeSessionKey,
    getStoredSessionKey,
    type GameLaunchParams,
    type SessionKeyPair,
} from '../utils/gameLauncher'
import { deriveGamePDA } from '../utils/pda'

interface LaunchGameResult {
    success: boolean
    message: string
}

interface UseGameLauncherReturn {
    launchGame: (gameId: string, playerColor: 'white' | 'black', wagerAmount: number, opponentPubkey?: string) => Promise<LaunchGameResult>
    isLaunching: boolean
    sessionKey: SessionKeyPair | null
    getStoredSession: (gameId: string) => SessionKeyPair | null
}

/**
 * Hook for launching the game client from the web UI
 *
 * This hook handles:
 * - Session key generation
 * - Launch parameter preparation
 * - Game client launch
 * - Session key storage
 */
export function useGameLauncher(): UseGameLauncherReturn {
    const { publicKey } = useWallet()
    const [isLaunching, setIsLaunching] = useState(false)
    const [sessionKey, setSessionKey] = useState<SessionKeyPair | null>(null)

    /**
     * Launch the game client
     */
    const launchGame = useCallback(
        async (
            gameId: string,
            playerColor: 'white' | 'black',
            wagerAmount: number,
            opponentPubkey?: string,
        ): Promise<LaunchGameResult> => {
            if (!publicKey) {
                return {
                    success: false,
                    message: 'Please connect your wallet first',
                }
            }

            setIsLaunching(true)

            try {
                // Check for existing session key
                const existingSession = getStoredSessionKey(gameId)
                let currentSession: SessionKeyPair

                if (existingSession) {
                    console.log('[useGameLauncher] Using existing session key')
                    currentSession = existingSession
                } else {
                    // Generate new session key
                    console.log('[useGameLauncher] Generating new session key')
                    currentSession = generateSessionKey()
                    storeSessionKey(gameId, currentSession)
                }

                setSessionKey(currentSession)

                // Generate P2P node ID
                const nodeId = generateNodeId()

                // Get game PDA
                const gameIdBN = new BN(gameId)
                const [gamePDA] = deriveGamePDA(gameIdBN)

                // Build launch parameters
                const launchParams: GameLaunchParams = {
                    gameId,
                    playerColor,
                    sessionKey: currentSession.secretKey,
                    sessionPubkey: currentSession.publicKey,
                    nodeId,
                    rpcUrl: 'https://api.devnet.solana.com',
                    gamePDA: gamePDA.toBase58(),
                    wagerAmount,
                    opponentPubkey,
                }

                console.log('[useGameLauncher] Launch params:', launchParams)

                // Write launch params to file
                await writeLaunchParamsFile(launchParams)

                // Attempt to launch game client
                const launched = await launchGameClient(launchParams)

                if (launched) {
                    return {
                        success: true,
                        message: 'Game client launched! Check your taskbar.',
                    }
                } else {
                    return {
                        success: false,
                        message: 'Failed to launch game client automatically. Please run the game manually.',
                    }
                }
            } catch (error) {
                console.error('[useGameLauncher] Launch failed:', error)
                return {
                    success: false,
                    message: error instanceof Error ? error.message : 'Unknown error occurred',
                }
            } finally {
                setIsLaunching(false)
            }
        },
        [publicKey],
    )

    /**
     * Get stored session key for a game
     */
    const getStoredSession = useCallback((gameId: string): SessionKeyPair | null => {
        return getStoredSessionKey(gameId)
    }, [])

    return {
        launchGame,
        isLaunching,
        sessionKey,
        getStoredSession,
    }
}
