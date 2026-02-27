import { Link } from 'react-router-dom'
import { useWallet } from '@solana/wallet-adapter-react'

// Placeholder for game history data
interface GameHistory {
    gameId: string
    opponent: string
    wager: number
    result: 'win' | 'loss' | 'draw'
    date: number
    moves: number
}

export default function History() {
    const { connected, publicKey } = useWallet()

    // Placeholder - will fetch from chain using getProgramAccounts with filters
    const games: GameHistory[] = []

    const getResultColor = (result: string) => {
        switch (result) {
            case 'win':
                return 'var(--accent-green)'
            case 'loss':
                return 'var(--accent-red)'
            default:
                return 'var(--text-secondary)'
        }
    }

    if (!connected) {
        return (
            <div>
                <Link to="/" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                    Back
                </Link>
                <h2 className="section-title">Game History</h2>
                <div className="glass-card" style={{ textAlign: 'center', padding: '3rem' }}>
                    <p style={{ color: 'var(--text-secondary)' }}>
                        Connect your wallet to view your game history.
                    </p>
                </div>
            </div>
        )
    }

    return (
        <div>
            <Link to="/" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                Back
            </Link>

            <h2 className="section-title">Your Games</h2>
            <p style={{ color: 'var(--text-secondary)', marginBottom: '1.5rem' }}>
                Wallet: {publicKey?.toBase58().slice(0, 16)}...
            </p>

            {games.length === 0 ? (
                <div className="glass-card" style={{ textAlign: 'center', padding: '3rem' }}>
                    <p style={{ color: 'var(--text-secondary)', marginBottom: '1rem' }}>
                        No games found. Start playing to build your history!
                    </p>
                    <Link to="/lobby" className="btn btn-primary">
                        Find a Game
                    </Link>
                </div>
            ) : (
                <div>
                    {games.map((game) => (
                        <Link
                            key={game.gameId}
                            to={`/game/${game.gameId}`}
                            className="game-card"
                            style={{ textDecoration: 'none', color: 'inherit' }}
                        >
                            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                                <span style={{ fontWeight: 600, color: getResultColor(game.result) }}>
                                    {game.result.charAt(0).toUpperCase() + game.result.slice(1)}
                                </span>
                            </div>
                            <div>
                                <div style={{ fontSize: '0.875rem', fontWeight: 500 }}>
                                    vs {game.opponent.slice(0, 8)}...
                                </div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                    {game.moves} moves
                                </div>
                            </div>
                            <div className="wager">{game.wager} SOL</div>
                            <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                                {new Date(game.date * 1000).toLocaleDateString()}
                            </div>
                        </Link>
                    ))}
                </div>
            )}
        </div>
    )
}
