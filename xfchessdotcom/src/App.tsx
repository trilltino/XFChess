import { useState, useEffect, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { BrowserRouter as Router, Routes, Route, Link, useLocation, Navigate } from 'react-router-dom';
import PlayPage from './pages/play/play';
import { Tournaments } from './pages/tournaments/tournaments';
import { Home } from './pages/marketing/home';
import { Features } from './pages/marketing/features';
import { OrganizationSchema } from './components/StructuredData';
import { Menu, X } from 'lucide-react';
import { Footer } from './components/Footer';

import './index.css';

export default function App() {
    // The website is a marketing and information surface only. Wallet
    // connection, sign-in and account creation all live in the game client —
    // so there is deliberately no wallet-adapter or Privy provider here, and
    // nothing on this site ever asks a visitor for a signature.
    return (
        <Router>
            <AppContent />
        </Router>
    );
}


function AppContent() {
    const location = useLocation();

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
                    <Link to="/play" className="nav-link" onClick={() => setIsMenuOpen(false)}>Play</Link>
                    <Link to="/tournaments" className="nav-link" onClick={() => setIsMenuOpen(false)}>Tournaments</Link>
                    <Link to="/features" className="nav-link" onClick={() => setIsMenuOpen(false)}>Features</Link>

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

        </div>
    );
}

