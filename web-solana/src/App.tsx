import { useMemo, useState, useEffect, useRef } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { BrowserRouter as Router, Routes, Route, Link, useLocation, Navigate } from 'react-router-dom';
import { ConnectionProvider, WalletProvider, useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets';
import { WalletConnectWalletAdapter } from '@solana/wallet-adapter-walletconnect';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';
import { SolanaMobileWalletAdapter, createDefaultAddressSelector, createDefaultAuthorizationResultCache, createDefaultWalletNotFoundHandler } from '@solana-mobile/wallet-adapter-mobile';
import { clusterApiUrl } from '@solana/web3.js';
import { Players } from './pages/Players';
import { VerifyProfile } from './pages/VerifyProfile';
import PlayPage from './pages/Play';
import WSetup from './pages/WSetup';
import CompliancePage from './pages/Compliance';
import LegalPage from './pages/Legal';
import AntiCheatPage from './pages/AntiCheat';
import KycPage from './pages/Kyc';
import { SignIn } from './pages/SignIn';
import Launch from './pages/Launch';
import NewsRelease from './pages/NewsRelease';
import { Tournaments } from './pages/Tournaments';
import { ChessComputer } from './pages/ChessComputer';
import { Home } from './pages/Home';
import { Pvp } from './pages/Pvp';
import Spectate from './pages/Spectate';
import TournamentDetail from './pages/TournamentDetail';
import TournamentStandings from './pages/TournamentStandings';
import TournamentPlay from './pages/TournamentPlay';
import { ProfileViewer } from './pages/ProfileViewer';
import { getAnchorProgram, fetchPlayerProfile } from './lib/anchor_client';
import { loginWithEmail } from './lib/api';
import { Menu, X, ChevronDown, Loader2 } from 'lucide-react';
import { Footer } from './components/Footer';

const dropdownVariants = {
    hidden: { opacity: 0, y: -10 },
    visible: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: -10 }
};



// Default styles that can be overridden by your app
import '@solana/wallet-adapter-react-ui/styles.css';
import './index.css';

// Check if running in Tauri
const isTauri = typeof window !== 'undefined' && (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== undefined;

export default function App() {
    const network = WalletAdapterNetwork.Devnet;
    const endpoint = useMemo(() => clusterApiUrl(network), [network]);

    const wallets = useMemo(
        () => [
            new PhantomWalletAdapter(),
            new SolflareWalletAdapter(),
            // WalletConnect is essential for Tauri/Desktop apps to connect to mobile wallets
            new WalletConnectWalletAdapter({
                network: network,
                options: {
                    // Get a Project ID at https://cloud.walletconnect.com/
                    projectId: '66e133d368e7ec815db15024d2627e2b', // Using a placeholder ID
                    metadata: {
                        name: 'XFChess',
                        description: 'XFChess - Decentralized Chess on Solana',
                        url: 'https://xfchess.com',
                        icons: ['https://xfchess.com/logo.png'],
                    },
                },
            }),
            new SolanaMobileWalletAdapter({
                addressSelector: createDefaultAddressSelector(),
                appIdentity: {
                    name: 'XFChess',
                    uri: 'https://xfchess.com',
                    icon: 'logo.png',
                },
                authorizationResultCache: createDefaultAuthorizationResultCache(),
                cluster: network,
                onWalletNotFound: createDefaultWalletNotFoundHandler(),
            }),
        ],
        [network]
    );

    // Disable autoConnect in Tauri to prevent "WalletConnectionError" 
    // when extension providers aren't found in the standalone window.
    const autoConnect = !isTauri;

    return (
        <ConnectionProvider endpoint={endpoint}>
            <WalletProvider wallets={wallets} autoConnect={autoConnect}>
                <Router>
                    <AppContent />
                </Router>
            </WalletProvider>
        </ConnectionProvider>
    );
}


function AppContent() {
    const { connected, publicKey, disconnect } = useWallet();
    const { connection } = useConnection();
    const location = useLocation();

    const [isModalOpen, setIsModalOpen] = useState(false);
    const [isMenuOpen, setIsMenuOpen] = useState(false);
    const [isLoginModalOpen, setIsLoginModalOpen] = useState(false);
    const [username, setUsername] = useState<string | null>(null);
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [userEmail, setUserEmail] = useState<string | null>(null);
    const [isLegalOpen, setIsLegalOpen] = useState(false);
    const [isCommunityOpen, setIsCommunityOpen] = useState(false);
    const [isGameTypesOpen, setIsGameTypesOpen] = useState(false);
    const [navVisible, setNavVisible] = useState(true);
    const lastScrollY = useRef(0);
    const closeDropdowns = () => { setIsLegalOpen(false); setIsCommunityOpen(false); setIsGameTypesOpen(false); };

    // Check authentication status on mount
    useEffect(() => {
        const token = localStorage.getItem('xfchess_token');
        const email = localStorage.getItem('xfchess_email');
        const storedUsername = localStorage.getItem('xfchess_username');
        if (token && email) {
            setIsLoggedIn(true);
            setUserEmail(email);
            if (storedUsername) setUsername(storedUsername);
        }
    }, []);

    // Scroll detection for navbar fade
    useEffect(() => {
        const handleScroll = () => {
            const currentScrollY = window.scrollY;
            const isScrollingDown = currentScrollY > lastScrollY.current;
            const isNearTop = currentScrollY < 50;
            
            if (isNearTop) {
                setNavVisible(true);
            } else if (isScrollingDown) {
                setNavVisible(false);
            } else {
                setNavVisible(true);
            }
            
            lastScrollY.current = currentScrollY;
        };

        window.addEventListener('scroll', handleScroll, { passive: true });
        return () => window.removeEventListener('scroll', handleScroll);
    }, []);


    useEffect(() => {
        let isMounted = true;
        const load = async () => {
            if (!connected || !publicKey) {
                if (isMounted) setUsername(null);
                return;
            }
            try {
                // Cast to unknown then to a compatible structure for read-only profile fetching.
                // We use a manual cast here instead of importing the Wallet type to avoid runtime module export errors.
                const program = getAnchorProgram(connection, { publicKey } as unknown as { publicKey: typeof publicKey });
                const profile = await fetchPlayerProfile(program, publicKey);
                if (isMounted) {
                    if (profile && profile.data.username) {
                        setUsername(profile.data.username);
                    } else {
                        setUsername(null);
                    }
                }
            } catch (e) {
                console.error("Error loading navbar profile:", e);
                if (isMounted) setUsername(null);
            }
        };
        load();
        return () => { isMounted = false; };
    }, [connected, publicKey, connection]);


    return (
        <div className="app-container">
            <div className="onboarding-bg"></div>
            <nav className={`navbar ${isMenuOpen ? 'mobile-open' : ''} ${navVisible ? 'nav-visible' : 'nav-hidden'}`}>
                <div className="nav-mobile-row">
                    <Link to="/" className="nav-logo" onClick={() => setIsMenuOpen(false)}>
                        <span style={{ fontSize: '15px', fontWeight: 800, letterSpacing: '-0.04em' }}>
                            <span style={{ color: 'var(--primary)' }}>XF</span>
                            <span style={{ color: '#fff' }}>Chess</span>
                        </span>
                    </Link>
                    <button className="mobile-menu-toggle" onClick={() => setIsMenuOpen(!isMenuOpen)}>
                        {isMenuOpen ? <X size={24} /> : <Menu size={24} />}
                    </button>
                </div>
                
                <div className={`nav-links ${isMenuOpen ? 'active' : ''}`}>
                    <Link to="/home" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>Home</Link>
                    <Link to="/play" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }} style={{ color: 'var(--accent)', fontWeight: 700 }}>Play</Link>
                    <div className="nav-legal-dropdown">
                        <button className="nav-link dropdown-toggle" onClick={() => { setIsGameTypesOpen(v => !v); setIsCommunityOpen(false); setIsLegalOpen(false); }}>
                            Game Modes <ChevronDown size={14} className={`dropdown-icon ${isGameTypesOpen ? 'open' : ''}`} />
                        </button>
                        <AnimatePresence>
                            {isGameTypesOpen && (
                                <motion.div 
                                    className="nav-legal-dropdown-menu"
                                    variants={dropdownVariants}
                                    initial="hidden"
                                    animate="visible"
                                    exit="exit"
                                    transition={{ duration: 0.2 }}
                                >
                                    <Link to="/pvp" className="nav-legal-dropdown-item" onClick={() => { setIsGameTypesOpen(false); setIsMenuOpen(false); }}>PvP</Link>
                                    <Link to="/tournaments" className="nav-legal-dropdown-item" onClick={() => { setIsGameTypesOpen(false); setIsMenuOpen(false); }}>Tournament</Link>
                                    <Link to="/computer" className="nav-legal-dropdown-item" onClick={() => { setIsGameTypesOpen(false); setIsMenuOpen(false); }}>Chess Computer</Link>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>
                    <div className="nav-legal-dropdown">
                        <button className="nav-link dropdown-toggle" onClick={() => { setIsCommunityOpen(v => !v); setIsLegalOpen(false); setIsGameTypesOpen(false); }}>
                            Community <ChevronDown size={14} className={`dropdown-icon ${isCommunityOpen ? 'open' : ''}`} />
                        </button>
                        <AnimatePresence>
                            {isCommunityOpen && (
                                <motion.div 
                                    className="nav-legal-dropdown-menu"
                                    variants={dropdownVariants}
                                    initial="hidden"
                                    animate="visible"
                                    exit="exit"
                                    transition={{ duration: 0.2 }}
                                >
                                    <Link to="/players" className="nav-legal-dropdown-item" onClick={() => { setIsCommunityOpen(false); setIsMenuOpen(false); }}>Players</Link>
                                    <a href="https://t.me/+IBdo42qMPqM4Y2Vk" target="_blank" rel="noopener noreferrer" className="nav-legal-dropdown-item" onClick={() => { setIsCommunityOpen(false); setIsMenuOpen(false); }}>Telegram</a>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>
                    <div className="nav-legal-dropdown">
                        <button className="nav-link dropdown-toggle" onClick={() => { setIsLegalOpen(v => !v); setIsCommunityOpen(false); setIsGameTypesOpen(false); }}>
                            Legal <ChevronDown size={14} className={`dropdown-icon ${isLegalOpen ? 'open' : ''}`} />
                        </button>
                        <AnimatePresence>
                            {isLegalOpen && (
                                <motion.div
                                    className="nav-legal-dropdown-menu"
                                    variants={dropdownVariants}
                                    initial="hidden"
                                    animate="visible"
                                    exit="exit"
                                    transition={{ duration: 0.2 }}
                                >
                                    <Link to="/legal" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>Legal & Compliance</Link>
                                    <Link to="/anti-cheat" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>Anti-Cheat</Link>
                                    <Link to="/kyc" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>KYC</Link>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>
                    {isLoggedIn ? (
                        <button onClick={() => {
                            localStorage.removeItem('xfchess_token');
                            localStorage.removeItem('xfchess_email');
                            localStorage.removeItem('xfchess_username');
                            setIsLoggedIn(false);
                            setUserEmail(null);
                            setUsername(null);
                            setIsMenuOpen(false);
                        }} className="nav-link" style={{ fontSize: '12px', fontWeight: '600', letterSpacing: '0.04em' }}>
                            Logout
                        </button>
                    ) : (
                        <button onClick={() => { setIsLoginModalOpen(true); setIsMenuOpen(false); }} className="nav-link" style={{ fontSize: '12px', fontWeight: '600', letterSpacing: '0.04em' }}>
                            Login
                        </button>
                    )}
                    {connected && (
                        <Link to="/profile" className="nav-link" style={{ color: 'var(--accent)', fontWeight: 700 }} onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>
                            {username || "Set Name"}
                        </Link>
                    )}

                    <div className="nav-wallet-wrap">
                        {connected ? (
                            <button onClick={() => { disconnect(); setIsMenuOpen(false); }} className="btn-secondary disconnect-btn" style={{ height: '44px', width: '44px', padding: '0', borderRadius: '4px', border: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <X size={24} />
                            </button>
                        ) : (
                            <button onClick={() => { setIsModalOpen(true); setIsMenuOpen(false); }} className="nav-link" style={{ fontSize: '12px', fontWeight: '600', letterSpacing: '0.04em' }}>
                                Connect Wallet
                            </button>
                        )}
                    </div>
                </div>
            </nav>

            <div style={{ flex: 1 }}>
                <AnimatePresence mode="wait">
                    <Routes location={location} key={location.pathname}>
                        <Route path="/" element={<Navigate to="/home" replace />} />
                        <Route path="/home" element={<Home />} />
                        <Route path="/pvp" element={<Pvp />} />
                        <Route path="/players" element={<Players />} />
                        <Route path="/verify" element={<VerifyProfile />} />
                        <Route path="/play" element={<PlayPage />} />
                        <Route path="/w_setup" element={<WSetup />} />
                        <Route path="/compliance" element={<CompliancePage />} />
                        <Route path="/legal" element={<LegalPage />} />
                        <Route path="/anti-cheat" element={<AntiCheatPage />} />
                        <Route path="/profile" element={<ProfileViewer />} />
                        <Route path="/kyc" element={isLoggedIn ? <KycPage /> : <Navigate to="/login" replace />} />
                        <Route path="/news/release" element={<NewsRelease />} />
                        <Route path="/login" element={<SignIn defaultMode="login" />} />
                        <Route path="/auth/login" element={<SignIn defaultMode="login" />} />
                        <Route path="/launch" element={<Launch />} />
                        <Route path="/tournaments" element={<Tournaments />} />
                        <Route path="/tournament/:id" element={<TournamentDetail />} />
                        <Route path="/tournament/:id/standings" element={<TournamentStandings />} />
                        <Route path="/tournament/:id/play" element={<TournamentPlay />} />
                        <Route path="/spectate/:game_id" element={<Spectate />} />
                        <Route path="/computer" element={<ChessComputer />} />
                    </Routes>
                </AnimatePresence>
            </div>

            <Footer />

            {isModalOpen && <WalletSelectionModal onClose={() => setIsModalOpen(false)} />}
            {isLoginModalOpen && <LoginModal onClose={() => setIsLoginModalOpen(false)} onLoginSuccess={(email: string, username: string) => {
                setIsLoggedIn(true);
                setUserEmail(email);
                setUsername(username);
            }} />}
        </div>
    );
}

function WalletSelectionModal({ onClose }: { onClose: () => void }) {
    const { wallets, select } = useWallet();
    
    const descriptions: Record<string, string> = {
        'Phantom': isTauri ? 'Requires Chrome Extension (Browser only).' : 'The most popular Solana wallet with a sleek interface.',
        'Solflare': isTauri ? 'Requires Chrome Extension (Browser only).' : 'A powerful, feature-rich wallet with advanced security.',
        'WalletConnect': isTauri ? 'Recommended for Desktop App (Connect via Mobile).' : 'Connect to your mobile wallet via a secure bridge.',
        'Mobile Wallet Adapter': 'Native mobile connection for Android and iOS devices.',
    };

    // Sort wallets to prioritize WalletConnect in Tauri
    const sortedWallets = [...wallets].sort((a, b) => {
        if (isTauri) {
            if (a.adapter.name === 'WalletConnect') return -1;
            if (b.adapter.name === 'WalletConnect') return 1;
        }
        return 0;
    });

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div className="custom-wallet-modal" onClick={e => e.stopPropagation()}>
                <div className="modal-header">
                    <h3>Select Network Provider {isTauri && <span style={{ fontSize: '0.7rem', opacity: 0.6, background: 'var(--primary)', color: '#fff', padding: '2px 8px', borderRadius: '10px', marginLeft: '10px', verticalAlign: 'middle' }}>DESKTOP APP</span>}</h3>
                    <button className="modal-close" onClick={onClose}>&times;</button>
                </div>
                <div className="wallet-list">
                    {sortedWallets.map((wallet) => {
                        const isDisabled = isTauri && (wallet.adapter.name === 'Phantom' || wallet.adapter.name === 'Solflare');
                        const isRecommended = isTauri && wallet.adapter.name === 'WalletConnect';
                        
                        return (
                            <div 
                                key={wallet.adapter.name} 
                                className={`wallet-item ${isDisabled ? 'disabled' : ''} ${isRecommended ? 'recommended' : ''}`}
                                onClick={() => {
                                    if (isDisabled) return;
                                    select(wallet.adapter.name);
                                    onClose();
                                }}
                                style={{
                                    opacity: isDisabled ? 0.5 : 1,
                                    cursor: isDisabled ? 'not-allowed' : 'pointer',
                                    border: isRecommended ? '1px solid var(--primary)' : '1px solid var(--border)'
                                }}
                            >
                                <div className="wallet-icon-wrap">
                                    <img src={wallet.adapter.icon} alt={wallet.adapter.name} width={32} height={32} />
                                </div>
                                <div className="wallet-info">
                                    <h4 style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                        {wallet.adapter.name}
                                        {isRecommended && <span style={{ fontSize: '0.6rem', color: 'var(--primary)', fontWeight: 800 }}>RECOMMENDED</span>}
                                    </h4>
                                    <p>{descriptions[wallet.adapter.name] || 'Connect using your preferred Solana vault.'}</p>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

function LoginModal({ onClose, onLoginSuccess }: { onClose: () => void; onLoginSuccess: (email: string, username: string) => void }) {
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);
        if (!email || !password) {
            setError('Email and password are required');
            return;
        }
        setLoading(true);
        try {
            const res = await loginWithEmail({ email, password });
            localStorage.setItem('xfchess_token', res.token);
            localStorage.setItem('xfchess_username', res.username);
            localStorage.setItem('xfchess_email', email);
            onLoginSuccess(email, res.username);
            onClose();
        } catch (e: any) {
            setError(e.message || 'Login failed');
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div className="custom-wallet-modal" onClick={e => e.stopPropagation()} style={{ maxWidth: '400px' }}>
                <div className="modal-header">
                    <h3>Login</h3>
                    <button className="modal-close" onClick={onClose}>&times;</button>
                </div>
                <form onSubmit={handleSubmit} style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                    {error && <div style={{ color: '#ffd0d0', background: 'rgba(255, 80, 80, 0.12)', border: '1px solid rgba(255, 80, 80, 0.3)', borderRadius: '8px', padding: '12px', fontSize: '14px' }}>{error}</div>}
                    <div>
                        <label style={{ display: 'block', fontSize: '12px', fontWeight: 700, color: 'rgba(255,255,255,0.6)', marginBottom: '6px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Email</label>
                        <input
                            type="email"
                            value={email}
                            onChange={e => setEmail(e.target.value)}
                            placeholder="you@example.com"
                            style={{ width: '100%', padding: '12px 14px', borderRadius: '8px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff', fontSize: '14px', outline: 'none' }}
                        />
                    </div>
                    <div>
                        <label style={{ display: 'block', fontSize: '12px', fontWeight: 700, color: 'rgba(255,255,255,0.6)', marginBottom: '6px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Password</label>
                        <input
                            type="password"
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder="••••••••"
                            style={{ width: '100%', padding: '12px 14px', borderRadius: '8px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff', fontSize: '14px', outline: 'none' }}
                        />
                    </div>
                    <button
                        type="submit"
                        disabled={loading}
                        style={{ width: '100%', padding: '14px', borderRadius: '8px', border: 'none', background: 'linear-gradient(135deg, #ad5c2f, #8c4a26)', color: '#fff', fontWeight: 700, fontSize: '14px', cursor: loading ? 'not-allowed' : 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}
                    >
                        {loading ? <Loader2 size={16} className="spinner" /> : null}
                        {loading ? 'Signing in...' : 'Sign In'}
                    </button>
                </form>
            </div>
        </div>
    );
}
