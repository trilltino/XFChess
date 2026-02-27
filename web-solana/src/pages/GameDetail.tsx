import { useParams, Link } from 'react-router-dom'
import { useState, useEffect } from 'react'
import { useWallet } from '@solana/wallet-adapter-react'
import { useGameProgram, type GameData, type GameResult } from '../hooks/useGameProgram'
import { useGameLauncher } from '../hooks/useGameLauncher'
import { useMagicBlock } from '../hooks/useMagicBlock'
import { BN } from '@coral-xyz/anchor'
import { deriveGamePDA } from '../utils/pda'

interface TransactionResult {
    type: 'success' | 'error'
    message: string
    signature?: string
}

// Game Stage Step Component
function StageStep({
    number,
    title,
    description,
    status,
    action,
}: {
    number: number
    title: string
    description: string
    status: 'pending' | 'active' | 'completed'
    action?: React.ReactNode
}) {
    const colors = {
        pending: { bg: 'var(--bg-secondary)', border: 'var(--border-color)', text: 'var(--text-secondary)' },
        active: { bg: 'rgba(6, 182, 212, 0.1)', border: 'var(--accent-cyan)', text: 'var(--accent-cyan)' },
        completed: { bg: 'rgba(34, 197, 94, 0.1)', border: '#22c55e', text: '#22c55e' },
    }
    const color = colors[status]

    return (
        <div
            style={{
                display: 'flex',
                gap: '1rem',
                padding: '1.25rem',
                background: color.bg,
                border: `1px solid ${color.border}`,
                borderRadius: '12px',
                opacity: status === 'pending' ? 0.6 : 1,
            }}
        >
            <div
                style={{
                    width: '32px',
                    height: '32px',
                    borderRadius: '50%',
                    background: status === 'completed' ? '#22c55e' : status === 'active' ? 'var(--accent-cyan)' : 'var(--bg-secondary)',
                    color: 'white',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontWeight: 700,
                    fontSize: '0.875rem',
                    flexShrink: 0,
                }}
            >
                {status === 'completed' ? '✓' : number}
            </div>
            <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, color: color.text, marginBottom: '0.25rem' }}>{title}</div>
                <div style={{ fontSize: '0.875rem', color: 'var(--text-secondary)', marginBottom: action ? '0.75rem' : 0 }}>
                    {description}
                </div>
                {action}
            </div>
        </div>
    )
}

// Game Status Header
function GameStatusHeader({
    game,
    isPlayer,
    isWhite,
    isBlack,
}: {
    game: GameData
    isPlayer: boolean
    isWhite: boolean
    isBlack: boolean
}) {
    const statusConfig = {
        waiting: { icon: '⏳', color: '#eab308', label: 'Waiting for Opponent' },
        active: { icon: '▶', color: '#22c55e', label: 'Game in Progress' },
        finished: { icon: '✓', color: '#9ca3af', label: 'Game Finished' },
    }
    const config = statusConfig[game.status]

    return (
        <div
            style={{
                padding: '1.5rem',
                background: `linear-gradient(135deg, ${config.color}15, transparent)`,
                border: `1px solid ${config.color}30`,
                borderRadius: '12px',
                marginBottom: '1.5rem',
            }}
        >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '1rem' }}>
                <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.5rem' }}>
                        <span style={{ fontSize: '1.5rem' }}>{config.icon}</span>
                        <span style={{ fontSize: '1.25rem', fontWeight: 700, color: config.color }}>{config.label}</span>
                    </div>
                    <div style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
                        Game #{game.gameId.slice(0, 8)} • {game.moveCount} moves
                    </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                    <div style={{ fontSize: '2rem', fontWeight: 700, color: 'var(--accent-green)' }}>
                        {(game.wager * 2).toFixed(3)} SOL
                    </div>
                    <div style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>Total Pot</div>
                </div>
            </div>

            {isPlayer && (
                <div
                    style={{
                        marginTop: '1rem',
                        padding: '0.75rem',
                        background: 'rgba(255,255,255,0.05)',
                        borderRadius: '8px',
                        fontSize: '0.875rem',
                    }}
                >
                    You are playing as{' '}
                    <strong style={{ color: isWhite ? '#fff' : '#000', background: isWhite ? '#333' : '#ddd', padding: '0.125rem 0.5rem', borderRadius: '4px' }}>
                        {isWhite ? 'White' : 'Black'}
                    </strong>
                </div>
            )}
        </div>
    )
}

// Player Card
function PlayerCard({
    color,
    address,
    isUser,
}: {
    color: 'white' | 'black'
    address: string
    isUser: boolean
}) {
    return (
        <div
            style={{
                flex: 1,
                padding: '1.25rem',
                background: isUser ? 'rgba(6, 182, 212, 0.1)' : 'var(--bg-secondary)',
                border: `2px solid ${isUser ? 'var(--accent-cyan)' : color === 'white' ? '#444' : '#222'}`,
                borderRadius: '12px',
                textAlign: 'center',
            }}
        >
            <div
                style={{
                    width: '48px',
                    height: '48px',
                    borderRadius: '50%',
                    background: color === 'white' ? '#f5f5f5' : '#1a1a1a',
                    margin: '0 auto 0.75rem',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '1.5rem',
                    border: `2px solid ${color === 'white' ? '#ddd' : '#444'}`,
                }}
            >
                {color === 'white' ? '♔' : '♚'}
            </div>
            <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.25rem' }}>
                {color === 'white' ? 'White' : 'Black'} {isUser && '(You)'}
            </div>
            <div style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem' }}>
                {address.slice(0, 6)}...{address.slice(-4)}
            </div>
        </div>
    )
}

export default function GameDetail() {
    const { gameId } = useParams<{ gameId: string }>()
    const { publicKey, connected } = useWallet()
    const {
        fetchGame,
        finalizeGame,
        withdrawExpiredWager,
        isLoading,
        error,
        clearError,
        isReady,
        useMagicBlockER,
        toggleMagicBlockER,
    } = useGameProgram()
    const { launchGame, isLaunching } = useGameLauncher()
    const {
        status: erStatus,
        isDelegated,
        isLoading: isERLoading,
        error: erError,
        delegateGame,
        undelegateGame,
        checkStatus,
        clearError: clearERError,
    } = useMagicBlock()

    const [game, setGame] = useState<GameData | null>(null)
    const [isLoadingGame, setIsLoadingGame] = useState(true)
    const [result, setResult] = useState<TransactionResult | null>(null)
    const [launchResult, setLaunchResult] = useState<TransactionResult | null>(null)

    // Load game data
    useEffect(() => {
        const loadGame = async () => {
            if (!gameId || !isReady) return
            setIsLoadingGame(true)
            try {
                const gameBN = new BN(gameId)
                const [gamePDA] = deriveGamePDA(gameBN)
                const gameData = await fetchGame(gamePDA)
                if (gameData) setGame(gameData)
                await checkStatus(gameId)
            } catch (err) {
                console.error('Failed to load game:', err)
            } finally {
                setIsLoadingGame(false)
            }
        }
        loadGame()
    }, [gameId, isReady, fetchGame, checkStatus])

    // Handle delegate
    const handleDelegate = async () => {
        if (!gameId) return
        clearERError()
        try {
            await delegateGame(gameId)
            setResult({ type: 'success', message: 'Game delegated to ER successfully!' })
        } catch (err) {
            setResult({ type: 'error', message: err instanceof Error ? err.message : 'Failed to delegate' })
        }
    }

    // Handle undelegate
    const handleUndelegate = async () => {
        if (!gameId) return
        clearERError()
        try {
            await undelegateGame(gameId)
            setResult({ type: 'success', message: 'Game undelegated successfully!' })
        } catch (err) {
            setResult({ type: 'error', message: err instanceof Error ? err.message : 'Failed to undelegate' })
        }
    }

    // Handle ER toggle
    const handleERToggle = () => toggleMagicBlockER(!useMagicBlockER)

    // Handle finalize
    const handleFinalize = async (gameResult: GameResult) => {
        if (!game || !connected || !isReady) {
            setResult({ type: 'error', message: 'Not connected or game not loaded' })
            return
        }
        clearError()
        setResult(null)
        try {
            const gameBN = new BN(game.gameId)
            const finalizeResult = await finalizeGame(gameBN, gameResult)
            setResult({ type: 'success', message: 'Game finalized!', signature: finalizeResult.signature })
            const [gamePDA] = deriveGamePDA(gameBN)
            const updated = await fetchGame(gamePDA)
            if (updated) setGame(updated)
        } catch (err) {
            setResult({ type: 'error', message: err instanceof Error ? err.message : 'Failed to finalize' })
        }
    }

    // Handle withdraw
    const handleWithdraw = async () => {
        if (!game || !connected || !isReady) {
            setResult({ type: 'error', message: 'Not connected or game not loaded' })
            return
        }
        clearError()
        setResult(null)
        try {
            const gameBN = new BN(game.gameId)
            const withdrawResult = await withdrawExpiredWager(gameBN)
            setResult({ type: 'success', message: 'Wager withdrawn!', signature: withdrawResult.signature })
            const [gamePDA] = deriveGamePDA(gameBN)
            const updated = await fetchGame(gamePDA)
            if (updated) setGame(updated)
        } catch (err) {
            setResult({ type: 'error', message: err instanceof Error ? err.message : 'Failed to withdraw' })
        }
    }

    // Handle launch game
    const handleLaunchGame = async () => {
        if (!game || !connected || !gameId) {
            setLaunchResult({ type: 'error', message: 'Not connected or game not loaded' })
            return
        }
        const playerColor = publicKey?.toBase58() === game.white ? 'white' : 'black'
        const result = await launchGame(gameId, playerColor, game.wager)
        setLaunchResult({
            type: result.success ? 'success' : 'error',
            message: result.message,
        })
    }

    // Render notification
    const renderNotification = () => {
        if (!result && !error) return null
        const message = result?.message || error || ''
        const isError = result?.type === 'error' || !!error
        return (
            <div
                style={{
                    padding: '1rem 1.25rem',
                    marginBottom: '1.5rem',
                    borderRadius: '10px',
                    background: isError ? 'rgba(239, 68, 68, 0.15)' : 'rgba(34, 197, 94, 0.15)',
                    border: `1px solid ${isError ? 'var(--accent-red)' : '#22c55e'}`,
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.75rem',
                }}
            >
                <span style={{ fontSize: '1.25rem' }}>{isError ? '⚠️' : '✓'}</span>
                <span style={{ flex: 1 }}>{message}</span>
                {result?.signature && (
                    <a
                        href={`https://explorer.solana.com/tx/${result.signature}?cluster=devnet`}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={{ color: 'var(--accent-cyan)', fontSize: '0.875rem' }}
                    >
                        View →
                    </a>
                )}
            </div>
        )
    }

    if (isLoadingGame) {
        return (
            <div>
                <Link to="/lobby" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                    ← Back to Lobby
                </Link>
                <div style={{ textAlign: 'center', padding: '4rem' }}>
                    <span className="spinner" />
                    <p style={{ color: 'var(--text-secondary)', marginTop: '1rem' }}>Loading game...</p>
                </div>
            </div>
        )
    }

    if (!game) {
        return (
            <div>
                <Link to="/lobby" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                    ← Back to Lobby
                </Link>
                <div
                    style={{
                        textAlign: 'center',
                        padding: '3rem',
                        background: 'var(--bg-secondary)',
                        borderRadius: '12px',
                    }}
                >
                    <p style={{ color: 'var(--text-secondary)' }}>Game not found on-chain.</p>
                </div>
            </div>
        )
    }

    const isPlayer = publicKey && (game.white === publicKey.toBase58() || game.black === publicKey?.toBase58())
    const isWhite = publicKey?.toBase58() === game.white
    const isBlack = publicKey?.toBase58() === game.black
    const isCreator = isWhite
    const isOpponent = isBlack

    // Determine stage status
    const stage1Status = game.status !== 'waiting' ? 'completed' : isCreator ? 'active' : 'pending'
    const stage2Status =
        game.status === 'finished'
            ? 'completed'
            : game.status === 'active' && !isDelegated
                ? 'active'
                : isDelegated
                    ? 'completed'
                    : 'pending'
    const stage3Status = game.status === 'finished' ? 'completed' : game.status === 'active' ? 'active' : 'pending'
    const stage4Status = game.status === 'finished' ? 'completed' : 'pending'

    return (
        <div style={{ maxWidth: '800px', margin: '0 auto' }}>
            <Link to="/lobby" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                ← Back to Lobby
            </Link>

            {/* Game Status Header */}
            <GameStatusHeader game={game} isPlayer={!!isPlayer} isWhite={isWhite} isBlack={isBlack} />

            {/* Notifications */}
            {renderNotification()}

            {/* Players */}
            <div style={{ display: 'flex', gap: '1rem', marginBottom: '2rem' }}>
                <PlayerCard color="white" address={game.white} isUser={isWhite} />
                <div
                    style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        padding: '0 1rem',
                        fontSize: '1.25rem',
                        color: 'var(--text-secondary)',
                        fontWeight: 700,
                    }}
                >
                    VS
                </div>
                <PlayerCard
                    color="black"
                    address={game.black || 'Waiting...'}
                    isUser={isBlack}
                />
            </div>

            {/* Game Flow Stages */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', marginBottom: '2rem' }}>
                <h3 style={{ fontSize: '1rem', color: 'var(--text-secondary)', marginBottom: '0.5rem' }}>Game Progress</h3>

                {/* Stage 1: Matchmaking */}
                <StageStep
                    number={1}
                    title="Matchmaking"
                    description={
                        game.status === 'waiting'
                            ? isCreator
                                ? 'Waiting for an opponent to join your game...'
                                : 'This game is waiting for an opponent.'
                            : 'Opponent has joined. Match is ready!'
                    }
                    status={stage1Status}
                    action={
                        game.status === 'waiting' && isCreator ? (
                            <button onClick={handleWithdraw} disabled={isLoading} className="btn btn-secondary" style={{ fontSize: '0.875rem' }}>
                                {isLoading ? 'Withdrawing...' : 'Cancel & Withdraw Wager'}
                            </button>
                        ) : null
                    }
                />

                {/* Stage 2: Ephemeral Rollups */}
                {useMagicBlockER && (
                    <StageStep
                        number={2}
                        title="Enable Ephemeral Rollups"
                        description={
                            isDelegated
                                ? 'Game is running on ER for instant, gasless moves.'
                                : 'Delegate to Magic Block ER for faster gameplay. Skip this to play on mainnet (slower, costs gas per move).'
                        }
                        status={stage2Status}
                        action={
                            game.status === 'active' && isPlayer && !isDelegated ? (
                                <button
                                    onClick={handleDelegate}
                                    disabled={isERLoading}
                                    className="btn btn-primary"
                                    style={{ fontSize: '0.875rem' }}
                                >
                                    {isERLoading ? (
                                        <>
                                            <span className="spinner-small" /> Delegating...
                                        </>
                                    ) : (
                                        '⚡ Delegate to ER'
                                    )}
                                </button>
                            ) : isDelegated && isPlayer ? (
                                <button
                                    onClick={handleUndelegate}
                                    disabled={isERLoading}
                                    className="btn btn-secondary"
                                    style={{ fontSize: '0.875rem' }}
                                >
                                    {isERLoading ? (
                                        <>
                                            <span className="spinner-small" /> Undelegating...
                                        </>
                                    ) : (
                                        'Undelegate (End Game)'
                                    )}
                                </button>
                            ) : null
                        }
                    />
                )}

                {/* Stage 3: Play */}
                <StageStep
                    number={useMagicBlockER ? 3 : 2}
                    title="Play Game"
                    description={
                        game.status === 'active'
                            ? isPlayer
                                ? 'Launch the game client and make your moves. Game state syncs automatically.'
                                : 'Game is in progress. Watch the moves on the board.'
                            : game.status === 'finished'
                                ? 'Game completed. View the final result below.'
                                : 'Waiting for match to begin...'
                    }
                    status={stage3Status}
                    action={
                        game.status === 'active' && isPlayer ? (
                            <div>
                                {launchResult && (
                                    <div
                                        style={{
                                            padding: '0.75rem',
                                            marginBottom: '0.75rem',
                                            borderRadius: '6px',
                                            background: launchResult.type === 'error' ? 'rgba(239, 68, 68, 0.15)' : 'rgba(34, 197, 94, 0.15)',
                                            border: `1px solid ${launchResult.type === 'error' ? 'var(--accent-red)' : '#22c55e'}`,
                                            fontSize: '0.875rem',
                                        }}
                                    >
                                        {launchResult.type === 'error' ? '⚠️ ' : '✓ '}
                                        {launchResult.message}
                                    </div>
                                )}
                                <button
                                    onClick={handleLaunchGame}
                                    disabled={isLaunching}
                                    className="btn btn-primary"
                                    style={{
                                        fontSize: '0.875rem',
                                        background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
                                    }}
                                >
                                    {isLaunching ? (
                                        <>
                                            <span className="spinner-small" /> Launching...
                                        </>
                                    ) : (
                                        '🎮 Launch Game Client'
                                    )}
                                </button>
                            </div>
                        ) : null
                    }
                />

                {/* Stage 4: Finalize */}
                <StageStep
                    number={useMagicBlockER ? 4 : 3}
                    title="Finalize & Settle"
                    description={
                        game.status === 'finished'
                            ? 'Game has been finalized and wagers distributed.'
                            : 'When the game ends, either player can finalize to settle the wager and update ratings.'
                    }
                    status={stage4Status}
                    action={
                        game.status === 'active' && isPlayer ? (
                            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                                <button
                                    onClick={() => handleFinalize({ Winner: [publicKey!] })}
                                    disabled={isLoading}
                                    className="btn btn-primary"
                                    style={{ fontSize: '0.875rem' }}
                                >
                                    🏆 Claim Victory
                                </button>
                                <button
                                    onClick={() => handleFinalize({ Draw: {} })}
                                    disabled={isLoading}
                                    className="btn btn-secondary"
                                    style={{ fontSize: '0.875rem' }}
                                >
                                    🤝 Agree Draw
                                </button>
                            </div>
                        ) : null
                    }
                />
            </div>

            {/* ER Settings */}
            {isPlayer && game.status !== 'finished' && (
                <div
                    style={{
                        padding: '1rem',
                        background: 'var(--bg-secondary)',
                        borderRadius: '8px',
                        marginBottom: '1.5rem',
                    }}
                >
                    <label
                        style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.75rem',
                            cursor: 'pointer',
                        }}
                    >
                        <input
                            type="checkbox"
                            checked={useMagicBlockER}
                            onChange={handleERToggle}
                            disabled={isDelegated}
                        />
                        <div>
                            <div style={{ fontWeight: 600, fontSize: '0.875rem' }}>⚡ Use Ephemeral Rollups</div>
                            <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                {isDelegated
                                    ? 'Cannot disable while game is delegated'
                                    : 'Enable for gasless, instant gameplay'}
                            </div>
                        </div>
                    </label>
                </div>
            )}

            {/* Game Position */}
            <div
                style={{
                    padding: '1rem',
                    background: 'var(--bg-secondary)',
                    borderRadius: '8px',
                }}
            >
                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.5rem' }}>
                    Current Position (FEN)
                </div>
                <code
                    style={{
                        fontSize: '0.75rem',
                        wordBreak: 'break-all',
                        display: 'block',
                        padding: '0.75rem',
                        background: 'var(--bg-primary)',
                        borderRadius: '4px',
                        fontFamily: 'var(--font-mono)',
                    }}
                >
                    {game.fen}
                </code>
            </div>

            {/* ER Error */}
            {erError && (
                <div
                    style={{
                        padding: '1rem',
                        marginTop: '1rem',
                        borderRadius: '8px',
                        background: 'rgba(239, 68, 68, 0.15)',
                        border: '1px solid var(--accent-red)',
                        fontSize: '0.875rem',
                        color: '#ef4444',
                    }}
                >
                    ⚠️ {erError}
                </div>
            )}
        </div>
    )
}
