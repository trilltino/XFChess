import { useMemo, useState, useEffect, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { BrowserRouter as Router, Routes, Route, Link, useLocation, Navigate } from 'react-router-dom';
import { ConnectionProvider, WalletProvider, useWallet } from '@solana/wallet-adapter-react';
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';
import { SolanaMobileWalletAdapter, createDefaultAddressSelector, createDefaultAuthorizationResultCache, createDefaultWalletNotFoundHandler } from '@solana-mobile/wallet-adapter-mobile';
import { clusterApiUrl } from '@solana/web3.js';
import PlayPage from './pages/play/play';
import { Tournaments } from './pages/tournaments/tournaments';
import { Home } from './pages/marketing/home';
import { Features } from './pages/marketing/features';
import { OrganizationSchema } from './components/StructuredData';
import { Menu, X } from 'lucide-react';
import { Footer } from './components/Footer';
import { WalletSelectionModal } from './components/WalletSelectionModal';
import { PrivyProviderWrapper } from './privy/PrivyProviderWrapper';
import { PrivyStandardBridge } from './privy/PrivyStandardBridge';

// Default styles that can be overridden by your app
import '@solana/wallet-adapter-react-ui/styles.css';
import './index.css';

// Check if running in Tauri
const isTauri = typeof window !== 'undefined' && (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== undefined;

export default function App() {
    const network = WalletAdapterNetwork.Devnet;
    // api.devnet.solana.com is a shared, rate-limited, load-balanced public
    // cluster — reads against it can be inconsistent (an account that
    // genuinely exists on-chain intermittently comes back "does not exist"
    // from AccountClient.fetch depending on which backend node answers).
    // Prefer the already-configured Helius devnet endpoint when available;
    // fall back to the public URL so the app still works without a key set.
    const heliusKey = import.meta.env.VITE_HELIUS_API_KEY as string | undefined;
    const endpoint = useMemo(
        () => (heliusKey ? `https://devnet.helius-rpc.com/?api-key=${heliusKey}` : clusterApiUrl(network)),
        [network, heliusKey]
    );

    const wallets = useMemo(
        () => [
            new PhantomWalletAdapter(),
            new SolflareWalletAdapter(),
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

    // Privy sits OUTSIDE the wallet-adapter providers: it is the auth layer that
    // *produces* an embedded wallet, while wallet-adapter is the layer that
    // *consumes* it. The handoff is PrivyStandardBridge, which must therefore sit
    // INSIDE WalletProvider — it dispatches `wallet-standard:register-wallet`,
    // and wallet-adapter wraps the result in a StandardWalletAdapter so the
    // embedded wallet shows up in `useWallet().wallets` next to Phantom.
    //
    // With VITE_PRIVY_APP_ID unset both components render to nothing, so this
    // nesting is a no-op and the tree is exactly what it was before.
    return (
        <PrivyProviderWrapper rpcUrl={endpoint}>
            <ConnectionProvider endpoint={endpoint}>
                <WalletProvider wallets={wallets} autoConnect={autoConnect}>
                    <PrivyStandardBridge />
                    <Router>
                        <AppContent />
                    </Router>
                </WalletProvider>
            </ConnectionProvider>
        </PrivyProviderWrapper>
    );
}


function AppContent() {
    const { connected, disconnect } = useWallet();

    const location = useLocation();

    const [isModalOpen, setIsModalOpen] = useState(false);
    const [isMenuOpen, setIsMenuOpen] = useState(false);
    const [navVisible, setNavVisible] = useState(true);
    const lastScrollY = useRef(0);

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



    return (
        <div className="app-container">
            <OrganizationSchema />
            <div className="onboarding-bg"></div>
            <nav className={`navbar ${isMenuOpen ? 'mobile-open' : ''} ${navVisible ? 'nav-visible' : 'nav-hidden'}`}>
                <div className="nav-mobile-row">
                    <Link to="/" className="nav-logo" onClick={() => setIsMenuOpen(false)}>
                        <span style={{ fontSize: '14px', fontWeight: 700, letterSpacing: '0.06em' }}>
                            XFCHESS.COM
                        </span>
                    </Link>
                    <button className="mobile-menu-toggle" onClick={() => setIsMenuOpen(!isMenuOpen)}>
                        {isMenuOpen ? <X size={24} /> : <Menu size={24} />}
                    </button>
                </div>
                
                <div className={`nav-links ${isMenuOpen ? 'active' : ''}`}>
                    <Link to="/home" className="nav-link" onClick={() => setIsMenuOpen(false)}>Home</Link>
                    <Link to="/play" className="nav-link" onClick={() => setIsMenuOpen(false)} style={{ color: 'var(--accent)', fontWeight: 700 }}>Play</Link>
                    <Link to="/tournaments" className="nav-link" onClick={() => setIsMenuOpen(false)}>Tournaments</Link>
                    <Link to="/features" className="nav-link" onClick={() => setIsMenuOpen(false)}>Features</Link>

                    <div className="nav-wallet-wrap" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <span
                            title={connected ? 'Wallet connected' : 'Wallet not connected'}
                            style={{
                                width: '10px',
                                height: '10px',
                                borderRadius: '50%',
                                background: connected ? '#27c93f' : '#ff5f56',
                                flexShrink: 0,
                            }}
                        />
                        {connected ? (
                            <button onClick={() => {
                                disconnect();
                                setIsMenuOpen(false);
                            }} title="Disconnect wallet" aria-label="Disconnect wallet" className="btn-secondary disconnect-btn" style={{ height: '44px', width: '44px', padding: '0', borderRadius: '4px', border: 'none', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
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
                        <Route path="/play" element={<PlayPage />} />
                        <Route path="/tournaments" element={<Tournaments />} />
                        <Route path="/features" element={<Features />} />
                        {/* The site is these four routes. Everything else —
                            sign-in, profile, KYC, identity vault, wallet setup,
                            player lookup, spectate, legal, compliance,
                            anti-cheat, release notes, launch, and the
                            per-tournament detail/standings/play pages — was
                            removed deliberately; those paths now land here and
                            go home rather than rendering an empty shell. */}
                        <Route path="*" element={<Navigate to="/home" replace />} />
                    </Routes>
                </AnimatePresence>
            </div>

            <Footer />

            {isModalOpen && <WalletSelectionModal onClose={() => setIsModalOpen(false)} />}
        </div>
    );
}

// WalletSelectionModal lives in `./components/`.
