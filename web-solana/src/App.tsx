import { useMemo, useState, useEffect, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { ConnectionProvider, WalletProvider, useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets';
import { WalletConnectWalletAdapter } from '@solana/wallet-adapter-walletconnect';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';
import { SolanaMobileWalletAdapter, createDefaultAddressSelector, createDefaultAuthorizationResultCache, createDefaultWalletNotFoundHandler } from '@solana-mobile/wallet-adapter-mobile';
import { clusterApiUrl } from '@solana/web3.js';
import { Home } from './pages/Home';
import { Media } from './pages/Media';
import { Blog } from './pages/Blog';
import { ProfileViewer } from './pages/ProfileViewer';
import { VerifyProfile } from './pages/VerifyProfile';
import DownloadPage from './pages/Download';
import CompliancePage from './pages/Compliance';
import LegalPage from './pages/Legal';
import AntiCheatPage from './pages/AntiCheat';
import KycPage from './pages/Kyc';
import { SignIn } from './pages/SignIn';
import { SignUp } from './pages/SignUp';
import NewsRelease from './pages/NewsRelease';
import { getAnchorProgram, fetchPlayerProfile } from './lib/anchor_client';
import { Menu, X, ChevronDown } from 'lucide-react';
import { Footer } from './components/Footer';



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
    const [username, setUsername] = useState<string | null>(null);
    const [isLegalOpen, setIsLegalOpen] = useState(false);
    const [isCommunityOpen, setIsCommunityOpen] = useState(false);
    const [navVisible, setNavVisible] = useState(true);
    const lastScrollY = useRef(0);
    const closeDropdowns = () => { setIsLegalOpen(false); setIsCommunityOpen(false); };

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
                    <Link to="/" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>Home</Link>
                    <Link to="/download" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }} style={{ color: 'var(--accent)', fontWeight: 700 }}>Play</Link>
                    <Link to="/profile" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>Profile</Link>
                    <Link to="/auth/register" className="nav-link" onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>Sign Up</Link>
                    <div className="nav-legal-dropdown">
                        <button className="nav-link dropdown-toggle" onClick={() => setIsCommunityOpen(v => !v)}>
                            Community <ChevronDown size={14} className={`dropdown-icon ${isCommunityOpen ? 'open' : ''}`} />
                        </button>
                        {isCommunityOpen && (
                            <div className="nav-legal-dropdown-menu">
                                <Link to="/blog" className="nav-legal-dropdown-item" onClick={() => { setIsCommunityOpen(false); setIsMenuOpen(false); }}>Blog</Link>
                                <Link to="/media" className="nav-legal-dropdown-item" onClick={() => { setIsCommunityOpen(false); setIsMenuOpen(false); }}>Media</Link>
                            </div>
                        )}
                    </div>
                    <div className="nav-legal-dropdown">
                        <button className="nav-link dropdown-toggle" onClick={() => setIsLegalOpen(v => !v)}>
                            Legal <ChevronDown size={14} className={`dropdown-icon ${isLegalOpen ? 'open' : ''}`} />
                        </button>
                        {isLegalOpen && (
                            <div className="nav-legal-dropdown-menu">
                                <Link to="/legal" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>Legal & Compliance</Link>
                                <Link to="/anti-cheat" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>Anti-Cheat</Link>
                                <Link to="/kyc" className="nav-legal-dropdown-item" onClick={() => { setIsLegalOpen(false); setIsMenuOpen(false); }}>KYC</Link>
                            </div>
                        )}
                    </div>
                    {connected && (
                        <Link to="/profile" className="nav-link" style={{ color: 'var(--accent)', fontWeight: 700 }} onClick={() => { setIsMenuOpen(false); closeDropdowns(); }}>
                            {username || "Set Name"}
                        </Link>
                    )}

                    <div className="nav-wallet-wrap">
                        {connected ? (
                            <button onClick={() => { disconnect(); setIsMenuOpen(false); }} className="btn-secondary" style={{ height: '44px', padding: '0 20px', borderRadius: '4px', fontSize: '0.9rem', fontWeight: 700 }}>
                                Logout
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
                        <Route path="/" element={<Home />} />
                        <Route path="/blog" element={<Blog />} />
                        <Route path="/media" element={<Media />} />
                        <Route path="/profile" element={<ProfileViewer />} />
                        <Route path="/verify" element={<VerifyProfile />} />
                        <Route path="/download" element={<DownloadPage />} />
                        <Route path="/compliance" element={<CompliancePage />} />
                        <Route path="/legal" element={<LegalPage />} />
                        <Route path="/anti-cheat" element={<AntiCheatPage />} />
                        <Route path="/kyc" element={<KycPage />} />
                        <Route path="/news/release" element={<NewsRelease />} />
                        <Route path="/auth/login" element={<SignIn defaultMode="login" />} />
                        <Route path="/auth/register" element={<SignUp />} />
                    </Routes>
                </AnimatePresence>
            </div>

            <Footer />

            {isModalOpen && <WalletSelectionModal onClose={() => setIsModalOpen(false)} />}
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

