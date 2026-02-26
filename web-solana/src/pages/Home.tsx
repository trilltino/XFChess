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
                <p style={{ fontSize: '1.125rem', color: 'var(--text-secondary)', maxWidth: '600px', margin: '0 auto 2rem' }}>
                    Fully peer-to-peer chess with on-chain wagering. No central servers. No trusted intermediaries.
                    Just you, your opponent, and the blockchain.
                </p>

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

            <div style={{ marginTop: '3rem' }}>
                <h2 style={{ fontSize: '1.5rem', fontWeight: 700, marginBottom: '1.5rem', textAlign: 'center' }}>
                    How It Works
                </h2>

                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '1.5rem' }}>
                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            P2P NETWORKING
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>Iroh Gossip Network</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            Games are relayed through a peer-to-peer gossip network powered by Iroh nodes. No central server manages your game state. Your moves travel directly between players via encrypted channels.
                        </p>
                    </div>

                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            REAL-TIME SYNC
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>Braid Protocol</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            The Braid protocol handles real-time state synchronization between clients. Game state converges automatically even if players temporarily disconnect or messages arrive out of order.
                        </p>
                    </div>

                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            BLOCKCHAIN ESCROW
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>Solana Smart Contracts</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            Wagers are locked in on-chain escrow accounts via the XFChess program. The Solana program guarantees fair payouts based on game outcome or time expiration. No middleman holds your funds.
                        </p>
                    </div>

                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            MOVE VALIDATION
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>Shakmaty in Ephemeral Rollups</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            All moves are validated by the Shakmaty chess engine running inside MagicBlock Ephemeral Rollups. The Rust-based validator ensures only legal chess moves are accepted. Invalid moves are rejected at game speed before reaching Solana mainnet.
                        </p>
                    </div>

                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            CROSS-PLATFORM
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>Web and Native Clients</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            Play from your browser via the React web client or download the native Bevy engine client. Both connect to the same Iroh gossip network and Solana programs.
                        </p>
                    </div>

                    <div className="glass-card">
                        <div style={{ fontSize: '0.75rem', color: 'var(--accent-red)', fontWeight: 700, marginBottom: '0.5rem', letterSpacing: '0.05em' }}>
                            RANKINGS
                        </div>
                        <h3 style={{ fontSize: '1.125rem', fontWeight: 600, marginBottom: '0.5rem' }}>On-Chain ELO</h3>
                        <p style={{ fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                            Every game updates your ELO rating stored on-chain. Build a verifiable competitive record that follows your wallet across all XFChess clients.
                        </p>
                    </div>
                </div>
            </div>
        </div>
    )
}
