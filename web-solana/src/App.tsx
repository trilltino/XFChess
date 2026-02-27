import { Routes, Route, Link, Navigate, useLocation } from 'react-router-dom'
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui'
import { useWallet } from '@solana/wallet-adapter-react'
import { useEffect } from 'react'
import Home from './pages/Home'
import Lobby from './pages/Lobby'
import GameDetail from './pages/GameDetail'
import HistoryPage from './pages/History'

// Navigation Item Component
function NavItem({ to, children, active }: { to: string; children: React.ReactNode; active?: boolean }) {
    return (
        <Link
            to={to}
            style={{
                padding: '0.5rem 1rem',
                borderRadius: '6px',
                fontSize: '0.875rem',
                fontWeight: 500,
                color: active ? 'var(--accent-cyan)' : 'var(--text-secondary)',
                background: active ? 'rgba(6, 182, 212, 0.1)' : 'transparent',
                textDecoration: 'none',
                transition: 'all 0.2s',
            }}
        >
            {children}
        </Link>
    )
}

function Navbar() {
    const { connected, publicKey } = useWallet()
    const location = useLocation()

    return (
        <nav className="navbar">
            <div style={{ display: 'flex', alignItems: 'center', gap: '2rem' }}>
                <Link to="/" className="nav-logo">
                    <span>XF</span>Chess.
                </Link>
                {connected && (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <NavItem to="/lobby" active={location.pathname === '/lobby' || location.pathname === '/'}>
                            🎮 Lobby
                        </NavItem>
                        <NavItem to="/history" active={location.pathname === '/history'}>
                            📜 History
                        </NavItem>
                    </div>
                )}
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '1.5rem' }}>
                {connected && publicKey && (
                    <div
                        style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.5rem',
                            padding: '0.5rem 1rem',
                            background: 'var(--bg-secondary)',
                            borderRadius: '6px',
                            fontSize: '0.75rem',
                            fontFamily: 'var(--font-mono)',
                        }}
                    >
                        <span
                            style={{
                                width: '8px',
                                height: '8px',
                                borderRadius: '50%',
                                background: '#22c55e',
                            }}
                        />
                        {publicKey.toBase58().slice(0, 6)}...{publicKey.toBase58().slice(-4)}
                    </div>
                )}
                <WalletMultiButton />
            </div>
        </nav>
    )
}

// Redirect component that sends connected users to lobby
function HomeRedirect() {
    const { connected } = useWallet()

    if (connected) {
        return <Navigate to="/lobby" replace />
    }

    return <Home />
}

function App() {
    const { connected } = useWallet()

    // Update document title based on connection
    useEffect(() => {
        document.title = connected ? 'XFChess - Game Lobby' : 'XFChess - Play Chess on Solana'
    }, [connected])

    return (
        <div className="app-wrapper">
            <Navbar />
            <main className="main-content">
                <Routes>
                    <Route path="/" element={<HomeRedirect />} />
                    <Route path="/lobby" element={<Lobby />} />
                    <Route path="/game/:gameId" element={<GameDetail />} />
                    <Route path="/history" element={<HistoryPage />} />
                </Routes>
            </main>
        </div>
    )
}

export default App