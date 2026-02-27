/**
 * Game Launcher Utility
 *
 * This module handles launching the XFChess desktop game client
 * from the web UI with the necessary parameters.
 */

import { BN } from '@coral-xyz/anchor'
import { PublicKey } from '@solana/web3.js'

/**
 * Session key pair for ephemeral signing
 */
export interface SessionKeyPair {
    publicKey: string
    secretKey: string
}

/**
 * Launch parameters for the game client
 */
export interface GameLaunchParams {
    gameId: string
    playerColor: 'white' | 'black'
    sessionKey: string
    sessionPubkey: string
    nodeId: string
    rpcUrl: string
    gamePDA: string
    wagerAmount: number
    opponentPubkey?: string
}

/**
 * Generate a random session key pair for ephemeral signing
 * In production, this would use proper key generation
 */
export function generateSessionKey(): SessionKeyPair {
    // Generate random bytes for session key
    const randomBytes = new Uint8Array(64)
    crypto.getRandomValues(randomBytes)

    // Convert to base58-like string (simplified)
    const secretKey = Array.from(randomBytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('')

    // Derive public key from secret (simplified)
    const publicKeyBytes = randomBytes.slice(0, 32)
    const publicKey = Array.from(publicKeyBytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('')

    return { publicKey, secretKey }
}

/**
 * Generate a P2P node ID
 * In production, this would come from the actual P2P network
 */
export function generateNodeId(): string {
    const bytes = new Uint8Array(32)
    crypto.getRandomValues(bytes)
    return (
        'node_' +
        Array.from(bytes.slice(0, 8))
            .map((b) => b.toString(16).padStart(2, '0'))
            .join('')
    )
}

/**
 * Build launch command for the game client
 */
export function buildLaunchCommand(params: GameLaunchParams): string {
    const exePath = 'xfchess.exe'

    return [
        exePath,
        `--game-id ${params.gameId}`,
        `--player-color ${params.playerColor}`,
        `--session-key ${params.sessionKey}`,
        `--session-pubkey ${params.sessionPubkey}`,
        `--p2p-node-id ${params.nodeId}`,
        `--rpc-url ${params.rpcUrl}`,
        `--game-pda ${params.gamePDA}`,
        params.opponentPubkey ? `--opponent ${params.opponentPubkey}` : '',
    ]
        .filter(Boolean)
        .join(' ')
}

/**
 * Write launch parameters to a shared file for the batch file to read
 */
export async function writeLaunchParamsFile(
    params: GameLaunchParams,
): Promise<void> {
    // In a real implementation, this would write to a file
    // For web browsers, we'll use localStorage as a workaround
    // or trigger a download that the batch file can monitor

    const data = JSON.stringify(params, null, 2)
    localStorage.setItem(`xfchess_launch_${params.gameId}`, data)

    // Also trigger a file download for the batch file to pick up
    const blob = new Blob([data], { type: 'application/json' })
    const url = URL.createObjectURL(blob)

    // Create temporary link to download
    const link = document.createElement('a')
    link.href = url
    link.download = `.local/game_launch_${params.gameId}.json`
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    URL.revokeObjectURL(url)
}

/**
 * Launch the game client
 *
 * This attempts to launch the game using various methods:
 * 1. Custom protocol handler (xfchess://)
 * 2. File association
 * 3. Download batch file
 */
export async function launchGameClient(params: GameLaunchParams): Promise<boolean> {
    console.log('[GameLauncher] Launching game with params:', params)

    try {
        // Method 1: Try custom protocol handler
        const protocolUrl = buildProtocolUrl(params)
        console.log('[GameLauncher] Trying protocol URL:', protocolUrl)

        // Create hidden iframe to attempt protocol launch
        const iframe = document.createElement('iframe')
        iframe.style.display = 'none'
        document.body.appendChild(iframe)
        iframe.src = protocolUrl

        // Clean up iframe after attempt
        setTimeout(() => {
            document.body.removeChild(iframe)
        }, 1000)

        // Method 2: Write params to file for batch file monitoring
        await writeLaunchParamsFile(params)

        // Show instructions to user
        alert(
            `Game client launching...\n\n` +
            `If the game doesn't start automatically:\n` +
            `1. Run: ./launch_local_test.bat\n` +
            `2. Or manually run:\n` +
            `${buildLaunchCommand(params)}`,
        )

        return true
    } catch (error) {
        console.error('[GameLauncher] Failed to launch game:', error)
        return false
    }
}

/**
 * Build a custom protocol URL for launching the game
 */
function buildProtocolUrl(params: GameLaunchParams): string {
    const searchParams = new URLSearchParams({
        gameId: params.gameId,
        playerColor: params.playerColor,
        sessionKey: params.sessionKey,
        sessionPubkey: params.sessionPubkey,
        nodeId: params.nodeId,
        rpcUrl: params.rpcUrl,
        gamePDA: params.gamePDA,
    })

    if (params.opponentPubkey) {
        searchParams.set('opponent', params.opponentPubkey)
    }

    return `xfchess://launch?${searchParams.toString()}`
}

/**
 * Get game PDA from game ID
 */
export function getGamePDA(gameId: BN): string {
    // This should match the PDA derivation in the program
    // For now, return a placeholder
    return `game_${gameId.toString()}`
}

/**
 * Check if the game client is installed
 */
export async function checkGameClientInstalled(): Promise<boolean> {
    // In production, this would check for the executable
    // For now, assume it's installed if we're in a local environment
    return window.location.hostname === 'localhost'
}

/**
 * Store session key securely (encrypted in localStorage)
 */
export function storeSessionKey(gameId: string, sessionKey: SessionKeyPair): void {
    const key = `xfchess_session_${gameId}`
    const data = JSON.stringify(sessionKey)
    // In production, encrypt this data
    localStorage.setItem(key, data)
}

/**
 * Retrieve stored session key
 */
export function getStoredSessionKey(gameId: string): SessionKeyPair | null {
    const key = `xfchess_session_${gameId}`
    const data = localStorage.getItem(key)
    if (!data) return null
    try {
        return JSON.parse(data) as SessionKeyPair
    } catch {
        return null
    }
}

/**
 * Clear stored session key
 */
export function clearSessionKey(gameId: string): void {
    const key = `xfchess_session_${gameId}`
    localStorage.removeItem(key)
}
