import { useState } from 'react';
import { X } from 'lucide-react';
import { Link } from 'react-router-dom';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';


export function Tournaments() {
    const [showNotice, setShowNotice] = useState(true);

    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '100px' }}>
            <SeoHead meta={PAGE_METADATA.tournaments} />
            <div className="section-label">COMMUNITY</div>
            <h2 style={{ fontSize: '2.5rem', marginBottom: '8px' }}>Tournaments<span className="accent">.</span></h2>

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
                        <strong style={{ color: 'var(--primary)' }}>Wagering Requirements:</strong> Cash Tournaments require a Solana wallet and KYC verification.
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
                            Compete for Grand Prizes
                        </h3>
                        <p style={{ color: 'var(--text-dim)', lineHeight: 1.7, fontSize: '1.05rem' }}>
                            Join structured tournaments with guaranteed prize pools. Battle through brackets, climb the leaderboard, and claim your share of the winnings.
                        </p>
                    </div>
                </div>

                {/* Tournament Structure */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Tournament Structure
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
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Registration Phase</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Sign up for tournaments and deposit your entry fee. All entry fees are held in escrow until the tournament concludes.
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
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Bracket Play</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Compete in Swiss-system or elimination brackets. Each match is recorded on-chain with automatic result verification.
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
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Prize Distribution</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Winners receive their share of the prize pool instantly. Prize distribution is transparent and verifiable on-chain.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Entry Requirements */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Entry Requirements
                    </h3>
                    <div style={{ 
                        background: 'rgba(20, 241, 149, 0.05)', 
                        borderRadius: '12px', 
                        padding: '24px', 
                        border: '1px solid rgba(20, 241, 149, 0.2)' 
                    }}>
                        <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: '16px' }}>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>KYC Verification:</strong> Complete identity verification to participate in cash tournaments.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Solana Wallet:</strong> Connected wallet with sufficient SOL for entry fee and gas.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Minimum Rating:</strong> Some tournaments require a minimum Elo rating to ensure competitive balance.
                                </p>
                            </li>
                        </ul>
                    </div>
                </div>

                {/* Prize Distribution */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Prize Distribution
                    </h3>
                    <div style={{ background: 'rgba(255, 255, 255, 0.03)', borderRadius: '12px', padding: '32px', border: '1px solid rgba(255, 255, 255, 0.08)' }}>
                        <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: '20px' }}>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Instant Payout:</strong> Prize money transfers directly to winners' wallets immediately after tournament completion.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Transparent Distribution:</strong> Prize splits are defined in smart contracts and visible to all participants.
                                </p>
                            </li>
                            <li style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
                                <span style={{ color: '#ffffff', fontSize: '1.2rem' }}></span>
                                <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                    <strong style={{ color: '#fff' }}>Multiple Tiers:</strong> Grand tournaments offer larger prize pools, while daily tournaments provide frequent opportunities.
                                </p>
                            </li>
                        </ul>
                    </div>
                </div>

                {/* Tournament Types */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Tournament Types
                    </h3>
                    <div style={{ display: 'grid', gap: '16px' }}>
                        <div style={{ 
                            background: 'rgba(255, 255, 255, 0.03)', 
                            borderRadius: '12px', 
                            padding: '24px', 
                            border: '1px solid rgba(255, 255, 255, 0.08)' 
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Daily Tournaments</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Quick tournaments with smaller entry fees and prize pools. Perfect for daily practice and consistent earnings.
                            </p>
                        </div>
                        <div style={{ 
                            background: 'rgba(255, 255, 255, 0.03)', 
                            borderRadius: '12px', 
                            padding: '24px', 
                            border: '1px solid rgba(255, 255, 255, 0.08)' 
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Weekly Tournaments</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Mid-sized tournaments with guaranteed prize pools. Competitive brackets and larger stakes.
                            </p>
                        </div>
                        <div style={{ 
                            background: 'rgba(255, 255, 255, 0.03)', 
                            borderRadius: '12px', 
                            padding: '24px', 
                            border: '1px solid rgba(255, 255, 255, 0.10)' 
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Grand Tournaments</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Major events with substantial prize pools. Elite competition, extensive brackets, and significant rewards.
                            </p>
                        </div>
                    </div>
                </div>

                
            </div>
        </main>
    );
}



