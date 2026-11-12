import { useState, useEffect } from 'react';
import { BrowserRouter as Router, Routes, Route, Link, Navigate, useLocation } from 'react-router-dom';
import PlayPage from './pages/play/play';
import { Tournaments } from './pages/tournaments/tournaments';
import { Home } from './pages/marketing/home';
import { Features } from './pages/marketing/features';
import { OrganizationSchema } from './components/StructuredData';
import { Menu, X } from 'lucide-react';
import { Footer } from './components/Footer';

import './index.css';

export default function App() {
    // Wallet connection and account creation live in the game client.
    return (
        <Router>
            <AppContent />
        </Router>
    );
}


function AppContent() {
    const [isMenuOpen, setIsMenuOpen] = useState(false);
    const [navVisible, setNavVisible] = useState(true);
    const location = useLocation();

    // A link such as /#signup should land on its section, including from another page.
    useEffect(() => {
        if (!location.hash) return;
        document.getElementById(location.hash.slice(1))?.scrollIntoView({ behavior: 'smooth' });
    }, [location]);

    // Scroll detection for navbar fade
    useEffect(() => {
        const handleScroll = () => {
            setNavVisible(window.scrollY === 0);
        };

        window.addEventListener('scroll', handleScroll, { passive: true });
        return () => window.removeEventListener('scroll', handleScroll);
    }, []);




    return (
        <div className="app-container">
            <div className="bg-orbs" aria-hidden="true">
                <span className="bg-orb bg-orb-1" />
                <span className="bg-orb bg-orb-2" />
                <span className="bg-orb bg-orb-3" />
                <span className="bg-orb bg-orb-4" />
            </div>
            <OrganizationSchema />
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
                    <Link to="/play" className="nav-link" onClick={() => setIsMenuOpen(false)}>Play</Link>
                    <Link to="/features" className="nav-link" onClick={() => setIsMenuOpen(false)}>Features</Link>
                    <Link to="/tournaments" className="nav-link" onClick={() => setIsMenuOpen(false)}>Tournaments</Link>
                    <Link to={{ pathname: '/', hash: '#signup' }} className="nav-link" onClick={() => setIsMenuOpen(false)}>Sign up</Link>
                </div>
            </nav>

            <div style={{ flex: 1 }}>
                <Routes>
                    <Route path="/" element={<Home />} />
                    <Route path="/home" element={<Home />} />
                    <Route path="/play" element={<PlayPage />} />
                    <Route path="/tournaments" element={<Tournaments />} />
                    <Route path="/features" element={<Features />} />
                    <Route path="*" element={<Navigate to="/home" replace />} />
                </Routes>
            </div>

            <Footer />

        </div>
    );
}

