import { useEffect, useState, useCallback } from 'react'
import { useConnection, useWallet } from '@solana/wallet-adapter-react'
import { Program, AnchorProvider, BN, type Idl } from '@coral-xyz/anchor'
import { SystemProgram, type PublicKey } from '@solana/web3.js'
import idl from '../idl/xfchess_game.json'
import {
    PROGRAM_ID,
    deriveGamePDA,
    deriveMoveLogPDA,
    deriveEscrowPDA,
    deriveProfilePDA,
    deriveSessionDelegationPDA,
    deriveDelegationPDA,
    deriveDelegationMetadataPDA,
    deriveBufferPDA,
    generateGameId,
    solToLamports,
    DELEGATION_PROGRAM_ID,
    MAGIC_BLOCK_PROGRAM_ID,
} from '../utils/pda'

// Type definitions based on the IDL
type GameType = { PvP: {} } | { PvAI: {} }
type GameResult = { None: {} } | { Winner: [PublicKey] } | { Draw: {} }

interface GameData {
    gameId: string
    white: string
    black: string | null
    wager: number
    status: 'waiting' | 'active' | 'finished'
    fen: string
    moveCount: number
    turn: number
    createdAt: number
    updatedAt: number
    supportsER?: boolean
}

interface CreateGameResult {
    signature: string
    gameId: BN
    gamePDA: PublicKey
}

interface JoinGameResult {
    signature: string
    gamePDA: PublicKey
}

interface RecordMoveResult {
    signature: string
}

interface FinalizeGameResult {
    signature: string
}

interface WithdrawResult {
    signature: string
}

interface SessionKeyResult {
    signature: string
    sessionDelegationPDA: PublicKey
}

interface DelegateGameResult {
    signature: string
    delegationPDA: PublicKey
}

interface UndelegateGameResult {
    signature: string
}

export function useGameProgram() {
    const { connection } = useConnection()
    const wallet = useWallet()
    const [program, setProgram] = useState<Program<Idl> | null>(null)
    const [isLoading, setIsLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    // Magic Block ER state
    const [useMagicBlockER, setUseMagicBlockER] = useState(false)
    const [isDelegated, setIsDelegated] = useState(false)
    const [isDelegating, setIsDelegating] = useState(false)
    const [delegationError, setDelegationError] = useState<string | null>(null)

    // Initialize the Anchor program
    useEffect(() => {
        if (!wallet.publicKey || !wallet.signTransaction) {
            setProgram(null)
            return
        }

        const provider = new AnchorProvider(
            connection,
            wallet as any,
            { commitment: 'confirmed' }
        )

        const programInstance = new Program(
            idl as Idl,
            provider
        )

        setProgram(programInstance)
    }, [connection, wallet.publicKey, wallet.signTransaction])

    /**
     * Create a new game with a wager
     * @param wagerAmount - Amount in SOL to wager
     * @param gameType - 'pvp' or 'pvai'
     * @returns The transaction signature and game details
     */
    const createGame = useCallback(async (
        wagerAmount: number,
        gameType: 'pvp' | 'pvai'
    ): Promise<CreateGameResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Generate unique game ID
            const gameId = generateGameId()

            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [moveLogPDA] = deriveMoveLogPDA(gameId)
            const [escrowPDA] = deriveEscrowPDA(gameId)

            // Convert wager to lamports
            const wagerLamports = solToLamports(wagerAmount)

            // Prepare game type enum
            const gameTypeValue: GameType = gameType === 'pvp' ? { PvP: {} } : { PvAI: {} }

            // Build and send transaction
            const signature = await program.methods
                .createGame(gameId, wagerLamports, gameTypeValue)
                .accounts({
                    game: gamePDA,
                    moveLog: moveLogPDA,
                    escrowPda: escrowPDA,
                    player: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            return {
                signature,
                gameId,
                gamePDA,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Join an existing game as the second player
     * @param gameId - The game ID to join
     * @returns The transaction signature
     */
    const joinGame = useCallback(async (
        gameId: BN
    ): Promise<JoinGameResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [escrowPDA] = deriveEscrowPDA(gameId)

            // Build and send transaction
            const signature = await program.methods
                .joinGame(gameId)
                .accounts({
                    game: gamePDA,
                    escrowPda: escrowPDA,
                    player: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            return {
                signature,
                gamePDA,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Record a chess move on-chain
     * @param gameId - The game ID
     * @param moveStr - The move in algebraic notation (e.g., "e2e4")
     * @param nextFen - The FEN string after the move
     * @returns The transaction signature
     */
    const recordMove = useCallback(async (
        gameId: BN,
        moveStr: string,
        nextFen: string
    ): Promise<RecordMoveResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [moveLogPDA] = deriveMoveLogPDA(gameId)

            // Build and send transaction
            const signature = await program.methods
                .recordMove(gameId, moveStr, nextFen)
                .accounts({
                    game: gamePDA,
                    moveLog: moveLogPDA,
                    player: wallet.publicKey,
                })
                .rpc()

            return {
                signature,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Finalize a game and distribute the wager
     * @param gameId - The game ID
     * @param result - The game result (None, Winner, or Draw)
     * @returns The transaction signature
     */
    const finalizeGame = useCallback(async (
        gameId: BN,
        result: GameResult
    ): Promise<FinalizeGameResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [escrowPDA] = deriveEscrowPDA(gameId)

            // Fetch game to get player pubkeys for profile PDAs
            const programAny = program as any
            const gameAccount = await programAny.account.game.fetch(gamePDA)
            const whitePubkey = gameAccount.white as PublicKey
            const blackPubkey = gameAccount.black as PublicKey

            const [whiteProfilePDA] = deriveProfilePDA(whitePubkey)
            const [blackProfilePDA] = deriveProfilePDA(blackPubkey)

            // Build and send transaction
            const signature = await program.methods
                .finalizeGame(gameId, result)
                .accounts({
                    game: gamePDA,
                    whiteProfile: whiteProfilePDA,
                    blackProfile: blackProfilePDA,
                    whiteAuthority: whitePubkey,
                    blackAuthority: blackPubkey,
                    escrowPda: escrowPDA,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            return {
                signature,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Withdraw an expired wager (creator can withdraw if no one joined)
     * @param gameId - The game ID
     * @returns The transaction signature
     */
    const withdrawExpiredWager = useCallback(async (
        gameId: BN
    ): Promise<WithdrawResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [escrowPDA] = deriveEscrowPDA(gameId)

            // Build and send transaction
            const signature = await program.methods
                .withdrawExpiredWager(gameId)
                .accounts({
                    game: gamePDA,
                    escrowPda: escrowPDA,
                    player: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            return {
                signature,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Authorize a session key for a game (for gasless moves)
     * @param gameId - The game ID
     * @param sessionPubkey - The session key public key to authorize
     * @returns The transaction signature and delegation PDA
     */
    const authorizeSessionKey = useCallback(async (
        gameId: BN,
        sessionPubkey: PublicKey
    ): Promise<SessionKeyResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsLoading(true)
        setError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [sessionDelegationPDA] = deriveSessionDelegationPDA(gameId, wallet.publicKey)

            // Build and send transaction
            const signature = await program.methods
                .authorizeSessionKey(gameId, sessionPubkey)
                .accounts({
                    game: gamePDA,
                    sessionDelegation: sessionDelegationPDA,
                    player: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            return {
                signature,
                sessionDelegationPDA,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setError(errorMessage)
            throw err
        } finally {
            setIsLoading(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Fetch a game account by its PDA
     * @param gamePDA - The game's PDA
     * @returns The game data or null if not found
     */
    const fetchGame = useCallback(async (gamePDA: PublicKey): Promise<GameData | null> => {
        if (!program) return null

        try {
            // Use any to bypass type checking for dynamic IDL
            const programAny = program as any
            const gameAccount = await programAny.account.game.fetch(gamePDA)

            if (!gameAccount) return null

            // Map the on-chain data to our frontend format
            const status = gameAccount.status as any
            let statusStr: 'waiting' | 'active' | 'finished'

            if (status.waitingForOpponent !== undefined) {
                statusStr = 'waiting'
            } else if (status.active !== undefined) {
                statusStr = 'active'
            } else {
                statusStr = 'finished'
            }

            return {
                gameId: (gameAccount.gameId as BN).toString(),
                white: (gameAccount.white as PublicKey).toBase58(),
                black: gameAccount.black ? (gameAccount.black as PublicKey).toBase58() : null,
                wager: (gameAccount.wagerAmount as BN).toNumber() / 1e9,
                status: statusStr,
                fen: gameAccount.fen as string,
                moveCount: (gameAccount.moveCount as number) || 0,
                turn: (gameAccount.turn as number) || 0,
                createdAt: (gameAccount.createdAt as BN).toNumber(),
                updatedAt: (gameAccount.updatedAt as BN).toNumber(),
            }
        } catch {
            return null
        }
    }, [program])

    /**
     * Fetch all active games from the chain
     * Note: This uses getProgramAccounts which can be slow. 
     * For production, consider using an indexer like Helius or QuickNode.
     * @returns Array of game data
     */
    const fetchActiveGames = useCallback(async (): Promise<GameData[]> => {
        if (!program) return []

        try {
            const programAny = program as any
            const accounts = await programAny.account.game.all()

            return accounts
                .filter((acc: any) => {
                    const status = acc.account.status as any
                    // Only return waiting or active games
                    return status.waitingForOpponent !== undefined || status.active !== undefined
                })
                .map((acc: any) => {
                    const gameAccount = acc.account
                    const status = gameAccount.status as any
                    const statusStr: 'waiting' | 'active' | 'finished' =
                        status.waitingForOpponent !== undefined ? 'waiting' : 'active'

                    return {
                        gameId: (gameAccount.gameId as BN).toString(),
                        white: (gameAccount.white as PublicKey).toBase58(),
                        black: gameAccount.black ? (gameAccount.black as PublicKey).toBase58() : null,
                        wager: (gameAccount.wagerAmount as BN).toNumber() / 1e9,
                        status: statusStr,
                        fen: gameAccount.fen as string,
                        moveCount: (gameAccount.moveCount as number) || 0,
                        turn: (gameAccount.turn as number) || 0,
                        createdAt: (gameAccount.createdAt as BN).toNumber(),
                        updatedAt: (gameAccount.updatedAt as BN).toNumber(),
                    }
                })
        } catch (err) {
            console.error('Error fetching active games:', err)
            return []
        }
    }, [program])

    /**
     * Clear any error state
     */
    const clearError = useCallback(() => {
        setError(null)
    }, [])

    /**
     * Toggle Magic Block ER usage
     * @param enabled - Whether to enable ER
     */
    const toggleMagicBlockER = useCallback((enabled: boolean) => {
        setUseMagicBlockER(enabled)
    }, [])

    /**
     * Clear delegation error state
     */
    const clearDelegationError = useCallback(() => {
        setDelegationError(null)
    }, [])

    /**
     * Delegate a game to Magic Block Ephemeral Rollups
     * @param gameId - The game ID to delegate
     * @param validUntil - Unix timestamp when delegation expires (defaults to 24 hours from now)
     * @returns The transaction signature and delegation PDA
     */
    const delegateGame = useCallback(async (
        gameId: BN,
        validUntil?: BN
    ): Promise<DelegateGameResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsDelegating(true)
        setDelegationError(null)

        try {
            // Derive PDAs
            const [gamePDA] = deriveGamePDA(gameId)
            const [delegationPDA] = deriveDelegationPDA(gamePDA)
            const [delegationMetadataPDA] = deriveDelegationMetadataPDA(gamePDA)
            const [bufferPDA] = deriveBufferPDA(gamePDA)

            // Default valid_until is 24 hours from now
            const validUntilValue = validUntil || new BN(Math.floor(Date.now() / 1000) + 86400)

            // Build and send transaction
            const signature = await program.methods
                .delegateGame(gameId, validUntilValue)
                .accounts({
                    game: gamePDA,
                    payer: wallet.publicKey,
                    ownerProgram: PROGRAM_ID,
                    buffer: bufferPDA,
                    delegationRecord: delegationPDA,
                    delegationMetadata: delegationMetadataPDA,
                    delegationProgram: DELEGATION_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()

            setIsDelegated(true)

            return {
                signature,
                delegationPDA,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setDelegationError(errorMessage)
            throw err
        } finally {
            setIsDelegating(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Undelegate a game from Magic Block Ephemeral Rollups
     * @param gameId - The game ID to undelegate
     * @returns The transaction signature
     */
    const undelegateGame = useCallback(async (
        gameId: BN
    ): Promise<UndelegateGameResult> => {
        if (!program || !wallet.publicKey) {
            throw new Error('Wallet not connected')
        }

        setIsDelegating(true)
        setDelegationError(null)

        try {
            // Derive PDA
            const [gamePDA] = deriveGamePDA(gameId)

            // Build and send transaction
            const signature = await program.methods
                .undelegateGame(gameId)
                .accounts({
                    game: gamePDA,
                    payer: wallet.publicKey,
                    magicContext: MAGIC_BLOCK_PROGRAM_ID,
                    magicProgram: MAGIC_BLOCK_PROGRAM_ID,
                })
                .rpc()

            setIsDelegated(false)

            return {
                signature,
            }
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error occurred'
            setDelegationError(errorMessage)
            throw err
        } finally {
            setIsDelegating(false)
        }
    }, [program, wallet.publicKey])

    /**
     * Check if a game is delegated to ER
     * @param gameId - The game ID to check
     * @returns Whether the game is delegated
     */
    const checkDelegationStatus = useCallback(async (
        gameId: BN
    ): Promise<boolean> => {
        if (!program) return false

        try {
            const [gamePDA] = deriveGamePDA(gameId)
            const [delegationPDA] = deriveDelegationPDA(gamePDA)

            // Try to fetch the delegation account
            const accountInfo = await connection.getAccountInfo(delegationPDA)
            const isGameDelegated = accountInfo !== null
            setIsDelegated(isGameDelegated)
            return isGameDelegated
        } catch {
            setIsDelegated(false)
            return false
        }
    }, [program, connection])

    return {
        program,
        programId: PROGRAM_ID,
        isLoading,
        error,
        createGame,
        joinGame,
        recordMove,
        finalizeGame,
        withdrawExpiredWager,
        authorizeSessionKey,
        fetchGame,
        fetchActiveGames,
        clearError,
        isReady: !!program && wallet.connected,
        // Magic Block ER exports
        useMagicBlockER,
        toggleMagicBlockER,
        delegateGame,
        undelegateGame,
        checkDelegationStatus,
        isDelegated,
        isDelegating,
        delegationError,
        clearDelegationError,
    }
}

export type {
    GameData,
    CreateGameResult,
    JoinGameResult,
    RecordMoveResult,
    FinalizeGameResult,
    WithdrawResult,
    SessionKeyResult,
    GameResult,
    DelegateGameResult,
    UndelegateGameResult,
}
