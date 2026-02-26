import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useWallet, useConnection } from '@solana/wallet-adapter-react'

// Placeholder for game data type
interface GameData {
    gameId: string
    white: string
    black: string | null
    wager: number
    status: 'waiting' | 'active' | 'finished'
    createdAt: number
}

export default function Lobby() {
    const { connected } = useWallet()
    const [showCreateForm, setShowCreateForm] = useState(false)
    const [wagerAmount, setWagerAmount] = useState('0.1')
    const [gameType, setGameType] = useState<'pvp' | 'pvai'>('pvp')

    // Placeholder games list - will be fetched from chain
    const games: GameData[] = []

    const handleCreateGame = async () => {
        // TODO: Implement create_game transaction
        console.log('Creating game:', { wager: wagerAmount, type: gameType })
        setShowCreateForm(false)
    }

    const handleJoinGame = async (gameId: string) => {
        // TODO: Implement join_game transaction
        console.log('Joining game:', gameId)
    }

    if (showCreateForm) {
        return (
            <div>
                <button onClick={() => setShowCreateForm(false)} className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                    Back to Lobby
                </button>

                <h2 className="section-title">Create New Game</h2>
                <div className="glass-card" style={{ maxWidth: '500px' }}>
                    <div className="form-group">
                        <label className="form-label">Wager Amount (SOL)</label>
                        <input
                            type="number"
                            className="input"
                            value={wagerAmount}
                            onChange={(e) => setWagerAmount(e.target.value)}
                            min="0.001"
                            step="0.001"
                        />
                    </div>

                    <div className="form-group">
                        <label className="form-label">Game Type</label>
                        <div style={{ display: 'flex', gap: '1rem' }}>
                            <button
                                onClick={() => setGameType('pvp')}
                                className="btn"
                                style={{
                                    flex: 1,
                                    background: gameType === 'pvp' ? 'var(--accent-red)' : 'var(--bg-secondary)',
                                    color: gameType === 'pvp' ? 'white' : 'var(--text-primary)',
                                }}
                            >
                                PvP
                            </button>
                            <button
                                onClick={() => setGameType('pvai')}
                                className="btn"
                                style={{
                                    flex: 1,
                                    background: gameType === 'pvai' ? 'var(--accent-purple)' : 'var(--bg-secondary)',
                                    color: gameType === 'pvai' ? 'white' : 'var(--text-primary)',
                                }}
                            >
                                vs AI
                            </button>
                        </div>
                    </div>

                    <button onClick={handleCreateGame} className="btn btn-primary" style={{ width: '100%' }}>
                        Create Game & Deposit {wagerAmount} SOL
                    </button>
                </div>
            </div>
        )
    }

    return (
        <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1.5rem' }}>
                <h2 className="section-title">Game Lobby</h2>
                {connected && (
                    <button onClick={() => setShowCreateForm(true)} className="btn btn-primary">
                        Create Game
                    </button>
                )}
            </div>

            {!connected && (
                <div className="glass-card" style={{ textAlign: 'center', padding: '3rem' }}>
                    <p style={{ color: 'var(--text-secondary)' }}>
                        Connect your wallet to view open games and create matches.
                    </p>
                </div>
            )}

            {connected && games.length === 0 && (
                <div className="glass-card" style={{ textAlign: 'center', padding: '3rem' }}>
                    <p style={{ color: 'var(--text-secondary)', marginBottom: '1rem' }}>
                        No open games found. Create one to get started!
                    </p>
                    <button onClick={() => setShowCreateForm(true)} className="btn btn-primary">
                        Create First Game
                    </button>
                </div>
            )}

            {connected && games.length > 0 && (
                <div>
                    {games.map((game) => (
                        <div key={game.gameId} className="game-card">
                            <div>
                                <div className="game-id">Game #{game.gameId.slice(0, 8)}</div>
                                <div style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                                    Creator: {game.white.slice(0, 8)}...
                                </div>
                            </div>
                            <div className="wager">{game.wager} SOL</div>
                            <div className={`status status-${game.status}`}>
                                {game.status === 'waiting' ? 'Waiting' : game.status === 'active' ? 'Active' : 'Finished'}
                            </div>
                            <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                {new Date(game.createdAt * 1000).toLocaleDateString()}
                            </div>
                            {game.status === 'waiting' && (
                                <button onClick={() => handleJoinGame(game.gameId)} className="btn btn-primary">
                                    Join
                                </button>
                            )}
                        </div>
                    ))}
                </div>
            )}
        </div>
    )
}