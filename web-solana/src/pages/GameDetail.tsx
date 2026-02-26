import { useParams, Link } from 'react-router-dom'

export default function GameDetail() {
    const { gameId } = useParams<{ gameId: string }>()

    // Placeholder - will fetch game data from chain
    const game = {
        gameId: gameId || 'unknown',
        white: '11111111111111111111111111111111',
        black: '22222222222222222222222222222222',
        wager: 0.5,
        status: 'active' as const,
        moveCount: 24,
        fen: 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
        createdAt: Date.now() / 1000,
    }

    const handleFinalize = async () => {
        // TODO: Implement finalize_game transaction
        console.log('Finalizing game:', gameId)
    }

    const handleWithdraw = async () => {
        // TODO: Implement withdraw_expired_wager transaction
        console.log('Withdrawing wager:', gameId)
    }

    return (
        <div>
            <Link to="/lobby" className="btn btn-secondary" style={{ marginBottom: '1rem' }}>
                Back to Lobby
            </Link>

            <h2 className="section-title">Game #{game.gameId.slice(0, 8)}</h2>

            <div className="glass-card" style={{ maxWidth: '600px' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1.5rem', marginBottom: '1.5rem' }}>
                    <div>
                        <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.25rem' }}>
                            White
                        </div>
                        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '0.875rem' }}>
                            {game.white.slice(0, 12)}...
                        </div>
                    </div>
                    <div>
                        <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.25rem' }}>
                            Black
                        </div>
                        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '0.875rem' }}>
                            {game.black.slice(0, 12)}...
                        </div>
                    </div>
                </div>

                <div className="divider" />

                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '1rem', marginBottom: '1.5rem' }}>
                    <div style={{ textAlign: 'center' }}>
                        <div style={{ fontSize: '1.5rem', fontWeight: 700, color: 'var(--accent-green)' }}>
                            {game.wager * 2} SOL
                        </div>
                        <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>Pot</div>
                    </div>
                    <div style={{ textAlign: 'center' }}>
                        <div style={{ fontSize: '1.5rem', fontWeight: 700 }}>
                            {game.moveCount}
                        </div>
                        <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>Moves</div>
                    </div>
                    <div style={{ textAlign: 'center' }}>
                        <span className={`status-badge ${game.status}`}>
                            {game.status.charAt(0).toUpperCase() + game.status.slice(1)}
                        </span>
                    </div>
                </div>

                <div className="divider" />

                <div style={{ marginBottom: '1rem' }}>
                    <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginBottom: '0.5rem' }}>
                        Current Position (FEN)
                    </div>
                    <code style={{ fontSize: '0.75rem', wordBreak: 'break-all' }}>
                        {game.fen}
                    </code>
                </div>

                {game.status === 'active' && (
                    <button onClick={handleFinalize} className="btn btn-primary" style={{ width: '100%' }}>
                        Claim Victory & Finalize
                    </button>
                )}

                {game.status === 'waiting' && (
                    <button onClick={handleWithdraw} className="btn btn-secondary" style={{ width: '100%' }}>
                        Withdraw Expired Wager
                    </button>
                )}
            </div>
        </div>
    )
}
