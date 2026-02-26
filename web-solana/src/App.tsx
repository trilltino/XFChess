import { Routes, Route, Link } from 'react-router-dom'
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui'
import { useWallet } from '@solana/wallet-adapter-react'
import Home from './pages/Home'
import Lobby from './pages/Lobby'
import GameDetail from './pages/GameDetail'
import HistoryPage from './pages/History'

function Navbar() {
    const { connected, publicKey } = useWallet()

    return (
        <nav className="navbar">
            <Link to="/" className="nav-logo">
                <span>XF</span>Chess.
            </Link>
            <div style={{ display: 'flex', alignItems: 'center', gap: '1.5rem' }}>
                {connected && (
                    <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                        {publicKey?.toBase58().slice(0, 8)}...{publicKey?.toBase58().slice(-4)}
                    </span>
                )}
                <WalletMultiButton />
            </div>
        </nav>
    )
}

function Navigation() {
    return (
        <div style={{ display: 'flex', gap: '1rem', marginBottom: '2rem' }}>
            <Link to="/lobby" className="btn btn-primary">
                Game Lobby
            </Link>
            <Link to="/history" className="btn btn-secondary">
                My Games
            </Link>
        </div>
    )
}

function App() {
    return (
        <div>
            <Navbar />
            <div className="container">
                <Routes>
                    <Route path="/" element={<><Home /><Navigation /></>} />
                    <Route path="/lobby" element={<Lobby />} />
                    <Route path="/game/:gameId" element={<GameDetail />} />
                    <Route path="/history" element={<HistoryPage />} />
                </Routes>
            </div>
        </div>
    )
}

export default App