import { Link } from 'react-router-dom'
import { useWallet } from '@solana/wallet-adapter-react'
import GameLauncher from '../components/GameLauncher'

export default function Home() {
    const { connected } = useWallet()

    return (
        <div>
            <header style={{ textAlign: 'center', padding: '4rem 0' }}>
                <h1 style={{ fontSize: '3rem', fontWeight: 800, marginBottom: '1rem' }}>
                    Play Chess on <span style={{ color: 'var(--accent-red)' }}>Solana</span>
                </h1>
                {!connected ? (
                    <p style={{ color: 'var(--text-secondary)' }}>
                        Connect your wallet to get started
                    </p>
                ) : (
                    <Link to="/lobby" className="btn btn-primary" style={{ fontSize: '1rem', padding: '1rem 2rem' }}>
                        Enter Game Lobby
                    </Link>
                )}
            </header>

            {connected && (
                <div style={{ maxWidth: '400px', margin: '0 auto 3rem' }}>
                    <GameLauncher />
                </div>
            )}
        </div>
    )
}
