import { useState, useEffect } from 'react'
import { Link } from 'react-router-dom'
import { useWallet, useConnection } from '@solana/wallet-adapter-react'
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui'
import { useGameProgram, type CreateGameResult, type GameData } from '../hooks/useGameProgram'
import { lamportsToSol } from '../utils/pda'
import type { BN } from '@coral-xyz/anchor'

// Transaction result for display
interface TransactionResult {
    type: 'success' | 'error'
    message: string
    signature?: string
    gameId?: string
}

// Game status badge component
function StatusBadge({ status }: { status: string }) {
    const colors: Record<string, { bg: string; text: string; label: string }> = {
        waiting: { bg: 'rgba(234, 179, 8, 0.2)', text: '#eab308', label: '⏳ Waiting' },
        active: { bg: 'rgba(34, 197, 94, 0.2)', text: '#22c55e', label: '▶ Active' },
        finished: { bg: 'rgba(156, 163, 175, 0.2)', text: '#9ca3af', label: '✓ Finished' },
    }
    const style = colors[status] || colors.waiting

    return (
        <span
            style={{
                padding: '0.25rem 0.75rem',
                borderRadius: '20px',
                fontSize: '0.75rem',
                fontWeight: 600,
                background: style.bg,
                color: style.text,
            }}
        >
            {style.label}
        </span>
    )
}

// ERBadge component
function ERBadge() {
    return (
        <span
            style={{
                padding: '0.125rem 0.5rem',
                borderRadius: '4px',
                fontSize: '0.7rem',
                fontWeight: 600,
                background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
                color: 'white',
            }}
        >
            ⚡ ER
        </span>
    )
}

// Game Card Component
function GameCard({
    game,
    isUserGame,
    canJoin,
    onJoin,
    isJoining,
    publicKey,
}: {
    game: GameData
    isUserGame: boolean
    canJoin: boolean
    onJoin: () => void
    isJoining: boolean
    publicKey?: string
}) {
    const isWaiting = game.status === 'waiting'
    const isActive = game.status === 'active'
    const isFinished = game.status === 'finished'
    const isCreator = game.white === publicKey
    const isOpponent = game.black === publicKey

    return (
        <div
            className="game-card"
            style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '0.75rem',
                padding: '1.25rem',
                background: isUserGame
                    ? 'linear-gradient(135deg, rgba(6, 182, 212, 0.1), rgba(139, 92, 246, 0.05))'
                    : 'var(--bg-secondary)',
                border: isUserGame ? '1px solid rgba(6, 182, 212, 0.3)' : '1px solid var(--border-color)',
                borderRadius: '12px',
                transition: 'all 0.2s',
            }}
        >
            {/* Header */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.25rem' }}>
                        <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 700, fontSize: '1.1rem' }}>
                            #{game.gameId.slice(0, 8)}
                        </span>
                        {game.supportsER && <ERBadge />}
                    </div>
                    <StatusBadge status={game.status} />
                </div>
                <div style={{ textAlign: 'right' }}>
                    <div style={{ fontSize: '1.5rem', fontWeight: 700, color: 'var(--accent-green)' }}>
                        {game.wager.toFixed(3)} SOL
                    </div>
                    <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                        {isWaiting ? 'Entry Fee' : `Pot: ${(game.wager * 2).toFixed(3)} SOL`}
                    </div>
                </div>
            </div>

            {/* Players */}
            <div
                style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr auto 1fr',
                    gap: '0.75rem',
                    alignItems: 'center',
                    padding: '0.75rem',
                    background: 'var(--bg-primary)',
                    borderRadius: '8px',
                }}
            >
                <div style={{ textAlign: 'center' }}>
                    <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.25rem' }}>
                        White {isCreator && '(You)'}
                    </div>
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem' }}>
                        {game.white.slice(0, 6)}...{game.white.slice(-4)}
                    </div>
                </div>
                <div style={{ fontSize: '1.2rem', color: 'var(--text-secondary)' }}>VS</div>
                <div style={{ textAlign: 'center' }}>
                    <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.25rem' }}>
                        Black {isOpponent && '(You)'}
                    </div>
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem' }}>
                        {game.black ? (
                            `${game.black.slice(0, 6)}...${game.black.slice(-4)}`
                        ) : (
                            <span style={{ color: 'var(--accent-cyan)', fontStyle: 'italic' }}>Open</span>
                        )}
                    </div>
                </div>
            </div>

            {/* Actions */}
            <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.25rem' }}>
                {canJoin && (
                    <button
                        onClick={onJoin}
                        disabled={isJoining}
                        className="btn btn-primary"
                        style={{ flex: 1 }}
                    >
                        {isJoining ? (
                            <>
                                <span className="spinner-small" />
                                Joining...
                            </>
                        ) : (
                            '🎮 Join Game'
                        )}
                    </button>
                )}

                {isCreator && isWaiting && (
                    <Link
                        to={`/game/${game.gameId}`}
                        className="btn btn-secondary"
                        style={{ flex: 1, textAlign: 'center', textDecoration: 'none' }}
                    >
                        👁 View
                    </Link>
                )}

                {isCreator && isActive && (
                    <Link
                        to={`/game/${game.gameId}`}
                        className="btn btn-primary"
                        style={{
                            flex: 1,
                            textAlign: 'center',
                            textDecoration: 'none',
                            background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
                        }}
                    >
                        ▶ Play Now
                    </Link>
                )}

                {isOpponent && isActive && (
                    <Link
                        to={`/game/${game.gameId}`}
                        className="btn btn-primary"
                        style={{
                            flex: 1,
                            textAlign: 'center',
                            textDecoration: 'none',
                            background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
                        }}
                    >
                        ▶ Play Now
                    </Link>
                )}

                {(isFinished || (!isUserGame && !canJoin)) && (
                    <Link
                        to={`/game/${game.gameId}`}
                        className="btn btn-secondary"
                        style={{ flex: 1, textAlign: 'center', textDecoration: 'none' }}
                    >
                        👁 View Game
                    </Link>
                )}
            </div>
        </div>
    )
}

// Create Game Modal
function CreateGameModal({
    isOpen,
    onClose,
    onCreate,
    isLoading,
    balance,
    connected,
}: {
    isOpen: boolean
    onClose: () => void
    onCreate: (wager: string, gameType: 'pvp' | 'pvai', enableER: boolean) => void
    isLoading: boolean
    balance: number | null
    connected: boolean
}) {
    const [wagerAmount, setWagerAmount] = useState('0.1')
    const [gameType, setGameType] = useState<'pvp' | 'pvai'>('pvp')
    const [enableER, setEnableER] = useState(false)

    if (!isOpen) return null

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault()
        onCreate(wagerAmount, gameType, enableER)
    }

    return (
        <div
            style={{
                position: 'fixed',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                background: 'rgba(0, 0, 0, 0.85)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                zIndex: 1000,
                padding: '1rem',
            }}
            onClick={onClose}
        >
            <div
                style={{
                    background: 'var(--bg-primary)',
                    borderRadius: '16px',
                    border: '1px solid var(--border-color)',
                    maxWidth: '500px',
                    width: '100%',
                    maxHeight: '90vh',
                    overflow: 'auto',
                }}
                onClick={(e) => e.stopPropagation()}
            >
                <div
                    style={{
                        padding: '1.5rem',
                        borderBottom: '1px solid var(--border-color)',
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                    }}
                >
                    <h2 style={{ margin: 0, fontSize: '1.25rem' }}>Create New Game</h2>
                    <button
                        onClick={onClose}
                        style={{
                            background: 'none',
                            border: 'none',
                            color: 'var(--text-secondary)',
                            fontSize: '1.5rem',
                            cursor: 'pointer',
                        }}
                    >
                        ×
                    </button>
                </div>

                <form onSubmit={handleSubmit} style={{ padding: '1.5rem' }}>
                    {/* Balance Display */}
                    {connected && balance !== null && (
                        <div
                            style={{
                                marginBottom: '1.5rem',
                                padding: '1rem',
                                background: 'var(--bg-secondary)',
                                borderRadius: '8px',
                                display: 'flex',
                                justifyContent: 'space-between',
                                alignItems: 'center',
                            }}
                        >
                            <span style={{ color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
                                Your Balance
                            </span>
                            <span style={{ color: 'var(--accent-cyan)', fontWeight: 700, fontSize: '1.1rem' }}>
                                {balance.toFixed(4)} SOL
                            </span>
                        </div>
                    )}

                    {/* Wager Input */}
                    <div style={{ marginBottom: '1.5rem' }}>
                        <label
                            style={{
                                display: 'block',
                                marginBottom: '0.5rem',
                                fontSize: '0.875rem',
                                fontWeight: 600,
                            }}
                        >
                            Wager Amount (SOL)
                        </label>
                        <div style={{ display: 'flex', gap: '0.5rem', marginBottom: '0.5rem' }}>
                            {['0.01', '0.05', '0.1', '0.5', '1'].map((amount) => (
                                <button
                                    key={amount}
                                    type="button"
                                    onClick={() => setWagerAmount(amount)}
                                    style={{
                                        flex: 1,
                                        padding: '0.5rem',
                                        borderRadius: '6px',
                                        border: 'none',
                                        background: wagerAmount === amount ? 'var(--accent-cyan)' : 'var(--bg-secondary)',
                                        color: wagerAmount === amount ? 'white' : 'var(--text-primary)',
                                        fontSize: '0.875rem',
                                        cursor: 'pointer',
                                    }}
                                >
                                    {amount}
                                </button>
                            ))}
                        </div>
                        <input
                            type="number"
                            value={wagerAmount}
                            onChange={(e) => setWagerAmount(e.target.value)}
                            min="0.001"
                            step="0.001"
                            disabled={isLoading}
                            style={{
                                width: '100%',
                                padding: '0.75rem',
                                background: 'var(--bg-secondary)',
                                border: '1px solid var(--border-color)',
                                borderRadius: '8px',
                                color: 'var(--text-primary)',
                                fontSize: '1rem',
                            }}
                        />
                    </div>

                    {/* Game Type */}
                    <div style={{ marginBottom: '1.5rem' }}>
                        <label
                            style={{
                                display: 'block',
                                marginBottom: '0.5rem',
                                fontSize: '0.875rem',
                                fontWeight: 600,
                            }}
                        >
                            Game Type
                        </label>
                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                            <button
                                type="button"
                                onClick={() => setGameType('pvp')}
                                disabled={isLoading}
                                style={{
                                    padding: '1rem',
                                    borderRadius: '8px',
                                    border: `2px solid ${gameType === 'pvp' ? 'var(--accent-red)' : 'var(--border-color)'}`,
                                    background: gameType === 'pvp' ? 'rgba(230, 57, 70, 0.1)' : 'var(--bg-secondary)',
                                    color: 'var(--text-primary)',
                                    cursor: 'pointer',
                                    textAlign: 'left',
                                }}
                            >
                                <div style={{ fontWeight: 600, marginBottom: '0.25rem' }}>👥 PvP</div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                    Play against another player
                                </div>
                            </button>
                            <button
                                type="button"
                                onClick={() => setGameType('pvai')}
                                disabled={isLoading}
                                style={{
                                    padding: '1rem',
                                    borderRadius: '8px',
                                    border: `2px solid ${gameType === 'pvai' ? 'var(--accent-purple)' : 'var(--border-color)'}`,
                                    background:
                                        gameType === 'pvai' ? 'rgba(139, 92, 246, 0.1)' : 'var(--bg-secondary)',
                                    color: 'var(--text-primary)',
                                    cursor: 'pointer',
                                    textAlign: 'left',
                                }}
                            >
                                <div style={{ fontWeight: 600, marginBottom: '0.25rem' }}>🤖 vs AI</div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                    Play against Stockfish
                                </div>
                            </button>
                        </div>
                    </div>

                    {/* ER Toggle */}
                    <div
                        style={{
                            marginBottom: '1.5rem',
                            padding: '1rem',
                            background: enableER ? 'rgba(6, 182, 212, 0.1)' : 'var(--bg-secondary)',
                            borderRadius: '8px',
                            border: `1px solid ${enableER ? 'var(--accent-cyan)' : 'transparent'}`,
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
                                checked={enableER}
                                onChange={(e) => setEnableER(e.target.checked)}
                                disabled={isLoading}
                            />
                            <div>
                                <div style={{ fontWeight: 600, color: enableER ? 'var(--accent-cyan)' : undefined }}>
                                    ⚡ Enable Ephemeral Rollups
                                </div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                    Gasless, instant moves with Magic Block ER
                                </div>
                            </div>
                        </label>
                    </div>

                    {/* Submit Button */}
                    <button
                        type="submit"
                        disabled={isLoading || !connected}
                        className="btn btn-primary"
                        style={{ width: '100%', padding: '1rem', fontSize: '1rem' }}
                    >
                        {isLoading ? (
                            <>
                                <span className="spinner-small" />
                                Creating Game...
                            </>
                        ) : (
                            `Create Game & Deposit ${wagerAmount} SOL`
                        )}
                    </button>

                    {!connected && (
                        <p
                            style={{
                                textAlign: 'center',
                                marginTop: '0.75rem',
                                fontSize: '0.875rem',
                                color: 'var(--text-secondary)',
                            }}
                        >
                            Connect your wallet to create a game
                        </p>
                    )}
                </form>
            </div>
        </div>
    )
}

// Empty State Component
function EmptyState({ onCreate }: { onCreate: () => void }) {
    return (
        <div
            style={{
                textAlign: 'center',
                padding: '4rem 2rem',
                background: 'var(--bg-secondary)',
                borderRadius: '16px',
                border: '1px dashed var(--border-color)',
            }}
        >
            <div style={{ fontSize: '4rem', marginBottom: '1rem' }}>♟️</div>
            <h3 style={{ marginBottom: '0.5rem', color: 'var(--text-primary)' }}>No Games Available</h3>
            <p style={{ color: 'var(--text-secondary)', marginBottom: '1.5rem', maxWidth: '400px', margin: '0 auto 1.5rem' }}>
                The lobby is empty. Create the first game and set the stakes, or check back later for open matches.
            </p>
            <button onClick={onCreate} className="btn btn-primary" style={{ padding: '0.75rem 2rem' }}>
                Create First Game
            </button>
        </div>
    )
}

// Not Connected State
function NotConnectedState() {
    return (
        <div
            style={{
                textAlign: 'center',
                padding: '4rem 2rem',
                background: 'var(--bg-secondary)',
                borderRadius: '16px',
                border: '1px solid var(--border-color)',
            }}
        >
            <div style={{ fontSize: '4rem', marginBottom: '1rem' }}>🔌</div>
            <h3 style={{ marginBottom: '0.5rem' }}>Connect Your Wallet</h3>
            <p style={{ color: 'var(--text-secondary)', marginBottom: '1.5rem' }}>
                Connect your Solana wallet to view open games, create matches, and start playing.
            </p>
            <WalletMultiButton />
        </div>
    )
}

export default function Lobby() {
    const { connected, publicKey } = useWallet()
    const { connection } = useConnection()
    const {
        createGame,
        joinGame,
        fetchActiveGames,
        isLoading,
        error,
        clearError,
        isReady,
        toggleMagicBlockER,
    } = useGameProgram()

    // UI state
    const [showCreateModal, setShowCreateModal] = useState(false)
    const [balance, setBalance] = useState<number | null>(null)
    const [result, setResult] = useState<TransactionResult | null>(null)
    const [activeGames, setActiveGames] = useState<GameData[]>([])
    const [isLoadingGames, setIsLoadingGames] = useState(false)
    const [joiningGameId, setJoiningGameId] = useState<string | null>(null)
    const [dateCutoff, setDateCutoff] = useState<number>(7) // Days to show games from (0 = all)

    // Fetch wallet balance
    const fetchBalance = async () => {
        if (!publicKey || !connection) return
        try {
            const bal = await connection.getBalance(publicKey)
            setBalance(lamportsToSol(bal))
        } catch {
            setBalance(null)
        }
    }

    // Fetch active games
    const loadActiveGames = async () => {
        if (!connected) return
        setIsLoadingGames(true)
        try {
            const games = await fetchActiveGames()
            setActiveGames(games)
        } catch (err) {
            console.error('Failed to load games:', err)
        } finally {
            setIsLoadingGames(false)
        }
    }

    // Load data on mount
    useEffect(() => {
        if (connected) {
            fetchBalance()
            loadActiveGames()
        }
    }, [connected])

    // Handle create game
    const handleCreateGame = async (wagerAmount: string, gameType: 'pvp' | 'pvai', enableER: boolean) => {
        clearError()
        setResult(null)

        const wager = parseFloat(wagerAmount)
        if (isNaN(wager) || wager <= 0) {
            setResult({ type: 'error', message: 'Please enter a valid wager amount' })
            return
        }

        if (!connected || !isReady) {
            setResult({ type: 'error', message: 'Please connect your wallet first' })
            return
        }

        if (balance !== null && wager > balance) {
            setResult({
                type: 'error',
                message: `Insufficient balance. You have ${balance.toFixed(4)} SOL`,
            })
            return
        }

        try {
            if (enableER) {
                toggleMagicBlockER(true)
            }

            const result: CreateGameResult = await createGame(wager, gameType)

            setResult({
                type: 'success',
                message: `Game created!${enableER ? ' Delegate to ER before playing.' : ''}`,
                signature: result.signature,
                gameId: result.gameId.toString(),
            })

            setShowCreateModal(false)
            fetchBalance()
            loadActiveGames()
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error'
            let userMessage = errorMessage
            if (errorMessage.includes('insufficient funds')) {
                userMessage = 'Insufficient SOL balance'
            } else if (errorMessage.includes('User rejected')) {
                userMessage = 'Transaction cancelled'
            }
            setResult({ type: 'error', message: userMessage })
        }
    }

    // Handle join game
    const handleJoinGame = async (game: GameData) => {
        if (!connected || !isReady) {
            setResult({ type: 'error', message: 'Please connect your wallet' })
            return
        }

        if (game.white === publicKey?.toBase58()) {
            setResult({ type: 'error', message: 'Cannot join your own game' })
            return
        }

        // Check if game is full (black is set and not the System Program placeholder)
        const SYSTEM_PROGRAM = '11111111111111111111111111111111'
        if (game.black && game.black !== SYSTEM_PROGRAM) {
            setResult({ type: 'error', message: 'Game is full' })
            return
        }

        if (balance !== null && game.wager > balance) {
            setResult({ type: 'error', message: `Need ${game.wager} SOL to join` })
            return
        }

        setJoiningGameId(game.gameId)
        clearError()
        setResult(null)

        try {
            const { BN } = await import('@coral-xyz/anchor')
            const gameId = new BN(game.gameId)
            const joinResult = await joinGame(gameId)

            setResult({
                type: 'success',
                message: `Joined game #${game.gameId.slice(0, 8)}!`,
                signature: joinResult.signature,
            })

            fetchBalance()
            loadActiveGames()
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error'
            setResult({ type: 'error', message: errorMessage })
        } finally {
            setJoiningGameId(null)
        }
    }

    // Filter games by date cutoff
    const now = Math.floor(Date.now() / 1000)
    const cutoffTimestamp = dateCutoff > 0 ? now - dateCutoff * 24 * 60 * 60 : 0
    const recentGames = activeGames.filter((g) => g.createdAt >= cutoffTimestamp)

    // Categorize games
    const myGames = recentGames.filter(
        (g) => g.white === publicKey?.toBase58() || g.black === publicKey?.toBase58()
    )
    const openGames = recentGames.filter(
        (g) => g.status === 'waiting' && g.white !== publicKey?.toBase58() && (!g.black || g.black === '11111111111111111111111111111111')
    )
    const otherGames = recentGames.filter((g) => !myGames.includes(g) && !openGames.includes(g))

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

    return (
        <div className="lobby-container">
            {/* Header */}
            <div
                style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    marginBottom: '2rem',
                    flexWrap: 'wrap',
                    gap: '1rem',
                }}
            >
                <div>
                    <h1 style={{ margin: '0 0 0.25rem 0', fontSize: '1.75rem' }}>🎮 Game Lobby</h1>
                    <p style={{ margin: 0, color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
                        Create matches, join games, and compete for SOL
                    </p>
                </div>
                {connected && (
                    <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
                        {/* Date Cutoff Filter */}
                        <select
                            value={dateCutoff}
                            onChange={(e) => setDateCutoff(Number(e.target.value))}
                            style={{
                                padding: '0.5rem 0.75rem',
                                borderRadius: '8px',
                                border: '1px solid var(--border-color)',
                                background: 'var(--bg-secondary)',
                                color: 'var(--text-primary)',
                                fontSize: '0.875rem',
                                cursor: 'pointer',
                            }}
                            title="Filter games by date"
                        >
                            <option value={1}>Last 24 hours</option>
                            <option value={7}>Last 7 days</option>
                            <option value={30}>Last 30 days</option>
                            <option value={0}>All time</option>
                        </select>
                        <button
                            onClick={loadActiveGames}
                            disabled={isLoadingGames}
                            className="btn btn-secondary"
                            style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}
                        >
                            {isLoadingGames ? <span className="spinner-small" /> : '🔄'}
                            Refresh
                        </button>
                        <button onClick={() => setShowCreateModal(true)} className="btn btn-primary">
                            ➕ Create Game
                        </button>
                    </div>
                )}
            </div>

            {/* Notifications */}
            {renderNotification()}

            {/* Main Content */}
            {!connected ? (
                <NotConnectedState />
            ) : isLoadingGames ? (
                <div style={{ textAlign: 'center', padding: '4rem' }}>
                    <span className="spinner" />
                    <p style={{ color: 'var(--text-secondary)', marginTop: '1rem' }}>Loading games...</p>
                </div>
            ) : recentGames.length === 0 ? (
                <EmptyState onCreate={() => setShowCreateModal(true)} />
            ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
                    {/* My Games Section */}
                    {myGames.length > 0 && (
                        <section>
                            <h2
                                style={{
                                    fontSize: '1.1rem',
                                    marginBottom: '1rem',
                                    color: 'var(--accent-cyan)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '0.5rem',
                                }}
                            >
                                <span>👤</span> Your Games ({myGames.length})
                            </h2>
                            <div
                                style={{
                                    display: 'grid',
                                    gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
                                    gap: '1rem',
                                }}
                            >
                                {myGames.map((game) => (
                                    <GameCard
                                        key={game.gameId}
                                        game={game}
                                        isUserGame={true}
                                        canJoin={false}
                                        onJoin={() => { }}
                                        isJoining={false}
                                        publicKey={publicKey?.toBase58()}
                                    />
                                ))}
                            </div>
                        </section>
                    )}

                    {/* Open Games Section */}
                    {openGames.length > 0 && (
                        <section>
                            <h2
                                style={{
                                    fontSize: '1.1rem',
                                    marginBottom: '1rem',
                                    color: 'var(--accent-green)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '0.5rem',
                                }}
                            >
                                <span>🚪</span> Open Games ({openGames.length})
                            </h2>
                            <div
                                style={{
                                    display: 'grid',
                                    gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
                                    gap: '1rem',
                                }}
                            >
                                {openGames.map((game) => (
                                    <GameCard
                                        key={game.gameId}
                                        game={game}
                                        isUserGame={false}
                                        canJoin={true}
                                        onJoin={() => handleJoinGame(game)}
                                        isJoining={joiningGameId === game.gameId}
                                        publicKey={publicKey?.toBase58()}
                                    />
                                ))}
                            </div>
                        </section>
                    )}

                    {/* Other Games Section */}
                    {otherGames.length > 0 && (
                        <section>
                            <h2
                                style={{
                                    fontSize: '1.1rem',
                                    marginBottom: '1rem',
                                    color: 'var(--text-secondary)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '0.5rem',
                                }}
                            >
                                <span>👀</span> Watching ({otherGames.length})
                            </h2>
                            <div
                                style={{
                                    display: 'grid',
                                    gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
                                    gap: '1rem',
                                }}
                            >
                                {otherGames.map((game) => (
                                    <GameCard
                                        key={game.gameId}
                                        game={game}
                                        isUserGame={false}
                                        canJoin={false}
                                        onJoin={() => { }}
                                        isJoining={false}
                                        publicKey={publicKey?.toBase58()}
                                    />
                                ))}
                            </div>
                        </section>
                    )}
                </div>
            )}

            {/* Create Game Modal */}
            <CreateGameModal
                isOpen={showCreateModal}
                onClose={() => {
                    setShowCreateModal(false)
                    setResult(null)
                    clearError()
                }}
                onCreate={handleCreateGame}
                isLoading={isLoading}
                balance={balance}
                connected={connected}
            />
        </div>
    )
}
