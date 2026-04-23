import { motion } from 'framer-motion';
import { ArrowLeft, X, Rocket, Download } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';

const PlayPage = () => {
  const [showNotice, setShowNotice] = useState(true);

  const launchGame = () => {
    // Attempt to launch via deep link protocol with auth context
    const token = localStorage.getItem('xfchess_token') || '';
    const username = localStorage.getItem('xfchess_username') || '';
    window.location.href = `xfchess://launch?token=${token}&username=${username}`;
  };

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section" style={{ position: 'relative' }}>
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Play XFChess</div>
        <h1 style={{ fontSize: '2.5rem', fontWeight: 900, marginBottom: '8px' }}>Ready to Move?</h1>
        <p style={{ color: 'var(--text-dim)', marginBottom: '32px' }}>Launch the desktop application to play wagering and tournament games.</p>

        {/* Launch / Download Buttons */}
        <div style={{ display: 'flex', gap: '16px', marginTop: '32px', flexWrap: 'wrap' }}>
          <button
            onClick={launchGame}
            style={{
              padding: '18px 36px',
              background: 'linear-gradient(135deg, #ad5c2f, #8c4a26)',
              color: '#fff',
              borderRadius: '10px',
              fontWeight: 800,
              fontSize: '1.1rem',
              border: 'none',
              cursor: 'pointer',
              minWidth: '220px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '12px',
              boxShadow: '0 10px 30px rgba(173, 92, 47, 0.3)'
            }}
          >
            <Rocket size={20} />
            Launch Desktop App
          </button>

          <div style={{ display: 'flex', gap: '12px' }}>
              <button
                style={{
                  padding: '16px 24px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#fff',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px'
                }}
              >
                <Download size={18} />
                Windows
              </button>
              <button
                style={{
                  padding: '16px 24px',
                  background: 'rgba(255, 255, 255, 0.05)',
                  color: '#fff',
                  borderRadius: '10px',
                  fontWeight: 700,
                  fontSize: '0.9rem',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px'
                }}
              >
                <Download size={18} />
                macOS
              </button>
          </div>
        </div>

        <div style={{ marginTop: '48px', padding: '24px', background: 'rgba(255,255,255,0.02)', borderRadius: '12px', border: '1px solid rgba(255,255,255,0.05)' }}>
            <h3 style={{ margin: '0 0 12px 0', fontSize: '1.1rem' }}>First time playing?</h3>
            <p style={{ margin: 0, color: 'var(--text-dim)', fontSize: '0.9rem', lineHeight: 1.6 }}>
                Download the XFChess desktop client for your operating system above. Once installed, you can launch the game directly from this page or your applications folder.
            </p>
        </div>

        {/* Floating Wagering Notice Tooltip */}
        {showNotice && (
          <div style={{
            position: 'fixed',
            right: '20px',
            top: '50%',
            transform: 'translateY(-50%)',
            width: '280px',
            padding: '20px',
            background: 'rgba(8, 26, 20, 0.95)',
            border: '1px solid rgba(173, 92, 47, 0.3)',
            borderRadius: '12px',
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4)',
            backdropFilter: 'blur(16px)',
            zIndex: 1000
          }}>
            <button
              onClick={() => setShowNotice(false)}
              style={{
                position: 'absolute',
                top: '8px',
                right: '8px',
                background: 'none',
                border: 'none',
                color: 'var(--text-dim)',
                cursor: 'pointer',
                padding: '4px'
              }}
            >
              <X size={16} />
            </button>
            <p style={{ margin: 0, fontSize: '0.85rem', color: 'var(--text-dim)', lineHeight: 1.6, marginBottom: '12px' }}>
              <strong style={{ color: 'var(--primary)' }}>Wagering Requirements:</strong> PvP wagering requires a Solana wallet and KYC verification.
            </p>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <Link to="/kyc" style={{ color: '#ad5c2f', fontWeight: 600, fontSize: '0.85rem' }}>Complete KYC</Link>
              <a href="https://solflare.com" target="_blank" rel="noopener noreferrer" style={{ color: '#ad5c2f', fontWeight: 600, fontSize: '0.85rem' }}>Create wallet on Solflare</a>
            </div>
          </div>
        )}
      </section>
    </motion.div>
  );
};

export default PlayPage;
