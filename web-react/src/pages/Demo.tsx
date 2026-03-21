import { motion } from 'framer-motion';
import { ArrowLeft, Monitor, Wallet, Play, Download, Zap, Shield, Gamepad2, ChevronDown } from 'lucide-react';
import { Link } from 'react-router-dom';

const DemoPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Demo</div>
        <h2>XFChess <span className="accent">Demo</span></h2>

        <p>Experience XFChess in different modes - from standalone desktop gameplay to blockchain-powered Solana integration.</p>

        {/* Demo Dropdown */}
        <div className="compliance-dropdown-container">
          <div className="demo-dropdown">
            <button className="dropdown-button">
              Select Demo Mode
              <ChevronDown size={20} />
            </button>
            <div className="dropdown-menu">
              <button className="dropdown-item">
                <Monitor size={16} /> Standalone
              </button>
              <button className="dropdown-item">
                <Wallet size={16} /> Solana
              </button>
            </div>
          </div>
        </div>

        {/* Standalone Demo Section */}
        <div className="demo-sections">
          <div className="demo-section">
            <div className="demo-header">
              <div className="demo-icon">
                <Monitor size={48} color="#e63946" />
              </div>
              <div className="demo-info">
                <h3>Standalone Demo</h3>
                <p>Desktop-native chess gaming experience with local multiplayer and AI opponents</p>
              </div>
            </div>

            <div className="demo-content">
              <div className="demo-features">
                <h4>Features</h4>
                <div className="feature-grid">
                  <div className="feature-card">
                    <Gamepad2 size={24} color="#e63946" />
                    <h5>Local Multiplayer</h5>
                    <p>Play against friends on the same device with intuitive controls</p>
                  </div>
                  <div className="feature-card">
                    <Zap size={24} color="#e63946" />
                    <h5>AI Opponents</h5>
                    <p>Challenge built-in AI with adjustable difficulty levels</p>
                  </div>
                  <div className="feature-card">
                    <Shield size={24} color="#e63946" />
                    <h5>Secure Gaming</h5>
                    <p>Offline gameplay with no internet connection required</p>
                  </div>
                </div>
              </div>

              <div className="demo-actions">
                <h4>Get Started</h4>
                <div className="action-buttons">
                  <button className="btn-primary">
                    <Download size={16} />
                    Download Standalone
                  </button>
                  <button className="btn-secondary">
                    <Play size={16} />
                    Play in Browser
                  </button>
                </div>
              </div>

              <div className="demo-requirements">
                <h4>System Requirements</h4>
                <div className="requirements-grid">
                  <div className="requirement-item">
                    <strong>OS:</strong> Windows 10/11, macOS 10.15+, Linux
                  </div>
                  <div className="requirement-item">
                    <strong>Memory:</strong> 4GB RAM minimum
                  </div>
                  <div className="requirement-item">
                    <strong>Storage:</strong> 500MB available space
                  </div>
                  <div className="requirement-item">
                    <strong>Graphics:</strong> OpenGL 3.3 compatible
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Solana Demo Section */}
          <div className="demo-section">
            <div className="demo-header">
              <div className="demo-icon">
                <Wallet size={48} color="#e63946" />
              </div>
              <div className="demo-info">
                <h3>Solana Demo</h3>
                <p>Blockchain-powered chess with wagering, NFT rewards, and decentralized gameplay</p>
              </div>
            </div>

            <div className="demo-content">
              <div className="demo-features">
                <h4>Blockchain Features</h4>
                <div className="feature-grid">
                  <div className="feature-card">
                    <Wallet size={24} color="#e63946" />
                    <h5>Solana Integration</h5>
                    <p>Fast, low-cost transactions on the Solana blockchain</p>
                  </div>
                  <div className="feature-card">
                    <Zap size={24} color="#e63946" />
                    <h5>Smart Contract Wagering</h5>
                    <p>Secure peer-to-peer betting with automated payouts</p>
                  </div>
                  <div className="feature-card">
                    <Shield size={24} color="#e63946" />
                    <h5>Provably Fair</h5>
                    <p>Transparent gameplay with on-chain verification</p>
                  </div>
                </div>
              </div>

              <div className="demo-actions">
                <h4>Connect Wallet</h4>
                <div className="wallet-options">
                  <button className="wallet-btn phantom">
                    <div className="wallet-icon">👻</div>
                    Phantom Wallet
                  </button>
                  <button className="wallet-btn solflare">
                    <div className="wallet-icon">☀️</div>
                    Solflare Wallet
                  </button>
                </div>
              </div>

              <div className="demo-requirements">
                <h4>Requirements</h4>
                <div className="requirements-grid">
                  <div className="requirement-item">
                    <strong>Wallet:</strong> Phantom, Solflare, or compatible Solana wallet
                  </div>
                  <div className="requirement-item">
                    <strong>SOL Balance:</strong> Minimum 0.1 SOL for gas fees
                  </div>
                  <div className="requirement-item">
                    <strong>Network:</strong> Solana Mainnet Beta or Devnet
                  </div>
                  <div className="requirement-item">
                    <strong>Browser:</strong> Chrome, Firefox, Safari, or Edge
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="divider" />

        <div className="demo-showcase">
          <h3>Experience <span className="accent">XFChess</span></h3>
          <p>Choose your preferred gaming mode and start playing chess the way you want.</p>
          
          <div className="showcase-comparison">
            <div className="comparison-card standalone">
              <h4>Standalone Mode</h4>
              <ul>
                <li>✓ No internet required</li>
                <li>✓ Instant gameplay</li>
                <li>✓ Local multiplayer</li>
                <li>✓ AI opponents</li>
                <li>✓ One-time purchase</li>
              </ul>
            </div>
            
            <div className="vs-divider">VS</div>
            
            <div className="comparison-card solana">
              <h4>Solana Mode</h4>
              <ul>
                <li>✓ Blockchain rewards</li>
                <li>✓ Peer-to-peer wagering</li>
                <li>✓ NFT collectibles</li>
                <li>✓ Global tournaments</li>
                <li>✓ Play-to-earn</li>
              </ul>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default DemoPage;
