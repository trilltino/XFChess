import { useState } from 'react';
import { X } from 'lucide-react';
import { Link } from 'react-router-dom';
import chessWageringUI from '../assets/chess-wagering-ui.png';

export function Pvp() {
    const [showNotice, setShowNotice] = useState(true);

    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '100px' }}>
            <div className="section-label">GAME MODES</div>
            <h2 style={{ fontSize: '2.5rem', marginBottom: '8px' }}>PvP Wagering<span className="accent">.</span></h2>

            {/* Floating Wagering Notice Tooltip */}
            {showNotice && (
                <div style={{
                    position: 'fixed',
                    right: '20px',
                    top: '50%',
                    transform: 'translateY(-50%)',
                    width: '280px',
                    padding: '20px',
                    background: 'rgba(0, 0, 0, 0.95)',
                    border: '1px solid rgba(255, 255, 255, 0.15)',
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
                        <Link to="/kyc" style={{ color: '#ffffff', fontWeight: 600, fontSize: '0.85rem' }}>Complete KYC</Link>
                        <a href="https://solflare.com" target="_blank" rel="noopener noreferrer" style={{ color: '#ffffff', fontWeight: 600, fontSize: '0.85rem' }}>Create wallet on Solflare</a>
                    </div>
                </div>
            )}

            {/* Content Section */}
            <div style={{ maxWidth: '1000px', margin: '0 auto', marginTop: '40px' }}>
                {/* Hero with Image */}
                <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '40px',
                    marginBottom: '60px',
                    background: 'rgba(255, 255, 255, 0.03)',
                    border: '1px solid rgba(255, 255, 255, 0.08)',
                    borderRadius: '16px',
                    padding: '40px'
                }}>
                    <div style={{ flex: '1' }}>
                        <h3 style={{ fontSize: '1.8rem', fontWeight: 800, marginBottom: '16px', color: '#fff' }}>
                            Challenge Opponents Directly
                        </h3>
                        <p style={{ color: 'var(--text-dim)', lineHeight: 1.7, fontSize: '1.05rem' }}>
                            Put your chess skills to the test in head-to-head wagered matches. Set your stakes, find opponents of similar skill level, and compete for real SOL prizes.
                        </p>
                    </div>
                    <div style={{ flex: '0 0 350px' }}>
                        <img
                            src={chessWageringUI}
                            alt="PvP Wagering Interface"
                            style={{ width: '100%', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }}
                        />
                    </div>
                </div>

                {/* How Wagering Works */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        How PvP Wagering Works
                    </h3>
                    <div style={{ background: 'rgba(255, 255, 255, 0.03)', borderRadius: '12px', padding: '32px', border: '1px solid rgba(255, 255, 255, 0.08)' }}>
                        <div style={{ display: 'grid', gap: '24px' }}>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{ 
                                    width: '40px', 
                                    height: '40px', 
                                    borderRadius: '50%', 
                                    background: 'var(--primary)', 
                                    display: 'flex', 
                                    alignItems: 'center', 
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>1</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Set Your Stake</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Both players deposit their wager into a smart contract escrow. The funds are locked until the game concludes.
                                    </p>
                                </div>
                            </div>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{ 
                                    width: '40px', 
                                    height: '40px', 
                                    borderRadius: '50%', 
                                    background: 'var(--primary)', 
                                    display: 'flex', 
                                    alignItems: 'center', 
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>2</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Play Your Match</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Complete your game with all moves recorded on-chain. The smart contract verifies the game result automatically.
                                    </p>
                                </div>
                            </div>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{ 
                                    width: '40px', 
                                    height: '40px', 
                                    borderRadius: '50%', 
                                    background: 'var(--primary)', 
                                    display: 'flex', 
                                    alignItems: 'center', 
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>3</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Instant Payout</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Winner receives the entire escrowed amount immediately. No waiting, no manual withdrawals - automatic settlement.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                {/* KYC Requirements */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        KYC Verification Required
                    </h3>
                    <div style={{ 
                        background: 'rgba(20, 241, 149, 0.05)', 
                        borderRadius: '12px', 
                        padding: '24px', 
                        border: '1px solid rgba(20, 241, 149, 0.2)' 
                    }}>
                        <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                            To participate in wagered PvP matches, you must complete KYC verification. This ensures fair play, prevents fraud, and complies with UK AML regulations. Your identity is verified once, then you can wager freely.
                        </p>
                        <div style={{ marginTop: '16px' }}>
                            <Link 
                                to="http://localhost:5173/kyc" 
                                style={{ 
                                    display: 'inline-block',
                                    padding: '12px 24px',
                                    background: 'var(--primary)',
                                    color: '#fff',
                                    borderRadius: '8px',
                                    fontWeight: 700,
                                    textDecoration: 'none'
                                }}
                            >
                                Complete KYC Now
                            </Link>
                        </div>
                    </div>
                </div>

                {/* How You Get Paid */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        How You Get Paid
                    </h3>
                    <div style={{ background: 'rgba(255, 255, 255, 0.03)', borderRadius: '12px', padding: '32px', border: '1px solid rgba(255, 255, 255, 0.08)' }}>
                        <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: '20px' }}>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Instant Settlement:</strong> Winnings transfer directly to your Solana wallet immediately upon game completion.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>No Withdrawal Requests:</strong> Your funds are always in your wallet. We never hold custody of player funds.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Transparent Transactions:</strong> All wager transactions are recorded on-chain and verifiable by anyone.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Zero Fees on Winnings:</strong> You receive 100% of the escrowed amount. Platform fees are paid separately by players.
                                </p>
                            </li>
                        </ul>
                    </div>
                </div>
            </div>
        </main>
    );
}



