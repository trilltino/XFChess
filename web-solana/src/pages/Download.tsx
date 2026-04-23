import { motion } from 'framer-motion';
import { ArrowLeft, X } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';

const DownloadPage = () => {
  const [showNotice, setShowNotice] = useState(true);

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section" style={{ position: 'relative' }}>
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Download</div>

        {/* Download Buttons */}
        <div style={{ display: 'flex', gap: '16px', marginTop: '32px', flexWrap: 'wrap' }}>
          <button
            style={{
              padding: '16px 32px',
              background: 'var(--primary)',
              color: '#fff',
              borderRadius: '8px',
              fontWeight: 700,
              fontSize: '1rem',
              border: 'none',
              cursor: 'pointer',
              minWidth: '180px'
            }}
          >
            Windows PC
          </button>
          <button
            style={{
              padding: '16px 32px',
              background: 'rgba(255, 255, 255, 0.1)',
              color: '#fff',
              borderRadius: '8px',
              fontWeight: 700,
              fontSize: '1rem',
              border: '1px solid rgba(255, 255, 255, 0.2)',
              cursor: 'pointer',
              minWidth: '180px'
            }}
          >
            MacOS
          </button>
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
              <Link to="http://localhost:5173/kyc" style={{ color: '#ad5c2f', fontWeight: 600, fontSize: '0.85rem' }}>Complete KYC</Link>
              <a href="https://solflare.com" target="_blank" rel="noopener noreferrer" style={{ color: '#ad5c2f', fontWeight: 600, fontSize: '0.85rem' }}>Create wallet on Solflare</a>
            </div>
          </div>
        )}
      </section>
    </motion.div>
  );
};

export default DownloadPage;
