import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import { SeoHead } from '../components/SeoHead';
import { PRIVATE_PAGE_METADATA } from '../lib/seo/metadata';

const WSetup = () => {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      className="content-wrap page-overlay"
    >
      <SeoHead meta={PRIVATE_PAGE_METADATA.wSetup} />
      <section className="section">
        <Link to="/download" className="back-btn">
          <ArrowLeft size={18} /> Back
        </Link>

        <div className="section-label">Wallet Setup</div>
        <h2>
          Playing For <span className="accent">Money</span>
        </h2>

        <p>
          To play XFChess for money (PvP wagering and Cash Tournaments), you need a Solana
          wallet and a completed KYC verification.
        </p>

        <h3 style={{ marginTop: '32px' }}>Step 1 — Install Solflare</h3>
        <p>
          Solflare is the recommended wallet for XFChess. Download it from{' '}
          <a
            href="https://solflare.com"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)' }}
          >
            solflare.com
          </a>
          .
        </p>

        <h3 style={{ marginTop: '32px' }}>Step 2 — Fund Your Wallet</h3>
        <p>
          Add SOL to your wallet using an on-ramp provider or by transferring from an exchange.
        </p>
        <div style={{ marginTop: '16px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
          <a
            href="https://buy.moonpay.com?currency=sol"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)', fontWeight: 600 }}
          >
            MoonPay
          </a>
          <a
            href="https://transak.com"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)', fontWeight: 600 }}
          >
            Transak
          </a>
          <a
            href="https://banxa.com"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)', fontWeight: 600 }}
          >
            Banxa
          </a>
          <a
            href="https://www.binance.com/en/buy-sol"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)', fontWeight: 600 }}
          >
            Binance
          </a>
          <a
            href="https://www.coinbase.com/price/solana"
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: 'var(--primary)', fontWeight: 600 }}
          >
            Coinbase
          </a>
        </div>

        <h3 style={{ marginTop: '32px' }}>Step 3 — Complete KYC</h3>
        <p>
          Verify your identity via our <Link to="/kyc" style={{ color: 'var(--primary)' }}>KYC page</Link> to
          unlock wagering and tournament entry.
        </p>
        <p style={{ fontSize: '0.9rem', color: 'var(--text-dim)', marginTop: '8px' }}>
          Note: KYC verification requires an email address and a connected Solana wallet.
        </p>

        <h3 style={{ marginTop: '32px' }}>Step 4 — Connect to XFChess</h3>
        <p>
          Launch the game, open the wallet connection dialog, and select Solflare. You are ready
          to play for money.
        </p>
      </section>
    </motion.div>
  );
};

export default WSetup;
