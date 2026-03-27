import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { HashRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { ChevronDown } from 'lucide-react';
import DownloadPage from './pages/Download';
import CompliancePage from './pages/Compliance';
import BusinessPage from './pages/Business';
import MembershipPage from './pages/Membership';
import DemoPage from './pages/Demo';
import LegalPage from './pages/Legal';
import AntiCheatPage from './pages/AntiCheat';
import KycPage from './pages/Kyc';
import TestPage from './pages/Test';
import EarlyAccessPage from './pages/EarlyAccess';
import ClubOfferPage from './pages/ClubOffer';
import CreatorPage from './pages/Creator';

const Navbar = () => {
  const [isLegalDropdownOpen, setIsLegalDropdownOpen] = useState(false);
  const [isDemoDropdownOpen, setIsDemoDropdownOpen] = useState(false);
  const [isRevenueDropdownOpen, setIsRevenueDropdownOpen] = useState(false);

  const closeAllDropdowns = () => {
    setIsLegalDropdownOpen(false);
    setIsDemoDropdownOpen(false);
    setIsRevenueDropdownOpen(false);
  };

  const handleLegalDropdownClick = () => {
    if (isLegalDropdownOpen) {
      setIsLegalDropdownOpen(false);
    } else {
      closeAllDropdowns();
      setIsLegalDropdownOpen(true);
    }
  };

  const handleDemoDropdownClick = () => {
    if (isDemoDropdownOpen) {
      setIsDemoDropdownOpen(false);
    } else {
      closeAllDropdowns();
      setIsDemoDropdownOpen(true);
    }
  };

  const handleRevenueDropdownClick = () => {
    if (isRevenueDropdownOpen) {
      setIsRevenueDropdownOpen(false);
    } else {
      closeAllDropdowns();
      setIsRevenueDropdownOpen(true);
    }
  };

  return (
    <nav className="navbar">
      <Link to="/" className="nav-logo">
        <span className="xf">XF</span>Chess.
      </Link>
      <div className="nav-links">
        <Link to="/download" className="nav-link">Download</Link>
        <div className="nav-legal-dropdown">
          <button 
            className="nav-link dropdown-toggle"
            onClick={handleLegalDropdownClick}
          >
            Legal <ChevronDown size={16} className={`dropdown-icon ${isLegalDropdownOpen ? 'open' : ''}`} />
          </button>
          {isLegalDropdownOpen && (
            <div className="nav-legal-dropdown-menu">
              <Link to="/legal" className="nav-legal-dropdown-item" onClick={() => setIsLegalDropdownOpen(false)}>
                Legal & Compliance
              </Link>
              <Link to="/anti-cheat" className="nav-legal-dropdown-item" onClick={() => setIsLegalDropdownOpen(false)}>
                Anti-Cheat
              </Link>
              <Link to="/kyc" className="nav-legal-dropdown-item" onClick={() => setIsLegalDropdownOpen(false)}>
                KYC
              </Link>
            </div>
          )}
        </div>
        <div className="nav-demo-dropdown">
          <button 
            className="nav-link dropdown-toggle"
            onClick={handleDemoDropdownClick}
          >
            Demo <ChevronDown size={16} className={`dropdown-icon ${isDemoDropdownOpen ? 'open' : ''}`} />
          </button>
          {isDemoDropdownOpen && (
            <div className="nav-demo-dropdown-menu">
              <Link to="/demo" className="nav-demo-dropdown-item" onClick={() => setIsDemoDropdownOpen(false)}>
                Standalone
              </Link>
              <Link to="/demo" className="nav-demo-dropdown-item" onClick={() => setIsDemoDropdownOpen(false)}>
                Solana
              </Link>
              <Link to="/test" className="nav-demo-dropdown-item" onClick={() => setIsDemoDropdownOpen(false)}>
                ER Test Results
              </Link>
            </div>
          )}
        </div>
        <div className="nav-revenue-dropdown">
          <button 
            className="nav-link dropdown-toggle"
            onClick={handleRevenueDropdownClick}
          >
            Revenue <ChevronDown size={16} className={`dropdown-icon ${isRevenueDropdownOpen ? 'open' : ''}`} />
          </button>
          {isRevenueDropdownOpen && (
            <div className="nav-revenue-dropdown-menu">
              <Link to="/business" className="nav-revenue-dropdown-item" onClick={() => setIsRevenueDropdownOpen(false)}>
                Profit Calculator
              </Link>
              <Link to="/membership" className="nav-revenue-dropdown-item" onClick={() => setIsRevenueDropdownOpen(false)}>
                Membership & Tiers
              </Link>
            </div>
          )}
        </div>
        <Link to="/compliance" className="nav-link">Compliance</Link>
        <Link to="/creator" className="nav-link">Creators</Link>
        <Link to="/club-offer" className="nav-link">Club Partners</Link>
        <Link to="/early-access" className="nav-link nav-cta">Early Access</Link>
      </div>
    </nav>
  );
};

const CyclingHero = () => {
  const words = ['Chess.', 'PlayFriends.', 'PlayFamily.', 'PlayLocal.', 'PlayGlobal.'];
  const [index, setIndex] = useState(0);
  const [settled, setSettled] = useState(false);

  useEffect(() => {
    if (settled) return;
    const interval = setInterval(() => {
      setIndex((prev) => {
        if (prev === words.length - 1) {
          setSettled(true);
          return 0;
        }
        return prev + 1;
      });
    }, 2500);
    return () => clearInterval(interval);
  }, [settled, words.length]);

  return (
    <section className="landing">
      <div className="landing-title">
        <span className="xf">XF</span>
        <span className="cycling">
          <AnimatePresence mode="wait">
            <motion.span
              key={settled ? 'final' : index}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              transition={{ duration: 0.4 }}
              className="cycling-word"
            >
              {settled ? 'Chess.' : words[index]}
            </motion.span>
          </AnimatePresence>
        </span>
        {!settled && <span className="cursor" />}
      </div>
      <div className="landing-subtitle">Competitive Chess</div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: settled ? 1 : 0, y: settled ? 0 : 10 }}
        className="landing-tagline"
      >
        <span>Play Anywhere. Own your History.</span>
      </motion.div>
    </section>
  );
};

const LandingPage = () => {
  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
      <CyclingHero />
    </motion.div>
  );
};

const App = () => {
  const location = useLocation();

  return (
    <div className="app-container">
      <div className="grid-bg" />
      <Navbar />
      <main>
        <AnimatePresence mode="wait">
          <Routes location={location} key={location.pathname}>
            <Route path="/" element={<LandingPage />} />
            <Route path="/download" element={<DownloadPage />} />
            <Route path="/compliance" element={<CompliancePage />} />
            <Route path="/legal" element={<LegalPage />} />
            <Route path="/anti-cheat" element={<AntiCheatPage />} />
            <Route path="/kyc" element={<KycPage />} />
            <Route path="/demo" element={<DemoPage />} />
            <Route path="/test" element={<TestPage />} />
            <Route path="/early-access" element={<EarlyAccessPage />} />
            <Route path="/club-offer" element={<ClubOfferPage />} />
            <Route path="/creator" element={<CreatorPage />} />
            <Route path="/business" element={<BusinessPage />} />
            <Route path="/membership" element={<MembershipPage />} />
          </Routes>
        </AnimatePresence>
      </main>

      <footer className="footer">
      </footer>
    </div>
  );
};

export default function AppWrapper() {
  return (
    <Router>
      <App />
    </Router>
  );
}
