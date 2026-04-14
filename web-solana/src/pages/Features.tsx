import { Shield, Zap, Globe, Code } from 'lucide-react';



export function Features() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <div className="section-label">PLATFORM HIGHLIGHTS</div>
            <h2 style={{ fontSize: '3rem' }}>Core Evolution<span className="accent">.</span></h2>
            <p style={{ maxWidth: '700px', fontSize: '1.2rem', marginBottom: '48px' }}>
                XFChess is more than just a game. It's a decentralized ecosystem built on the world's most performant blockchain.
            </p>

            <div className="features-grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '32px', marginTop: '48px' }}>
                <div className="feature-card" style={{ padding: '40px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '16px', transition: 'all 0.3s ease' }}>
                    <div style={{ marginBottom: '20px' }}><Shield color="var(--primary)" size={40} /></div>
                    <h3 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>Identity Vaulting</h3>
                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6 }}>CARF 2026 compliant off-chain identity verification. Your PII is never on-chain, keeping you safe and compliant.</p>
                </div>

                <div className="feature-card" style={{ padding: '40px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '16px', transition: 'all 0.3s ease' }}>
                    <div style={{ marginBottom: '20px' }}><Zap color="var(--accent)" size={40} /></div>
                    <h3 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>High Performance</h3>
                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6 }}>Built on Solana for sub-second finality. Experience near-instant moves and seamless gameplay transitions.</p>
                </div>

                <div className="feature-card" style={{ padding: '40px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '16px', transition: 'all 0.3s ease' }}>
                    <div style={{ marginBottom: '20px' }}><Globe color="#fff" size={40} /></div>
                    <h3 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>Global Ranking</h3>
                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6 }}>A permanent, immutable Elo rating visible to the entire world. Your skills are recorded forever on the ledger.</p>
                </div>

                <div className="feature-card" style={{ padding: '40px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '16px', transition: 'all 0.3s ease' }}>
                    <div style={{ marginBottom: '20px' }}><Code color="#fff" size={40} /></div>
                    <h3 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>Open Ecosystem</h3>
                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6 }}>Open-source integration. Anyone can build analytical tools or custom interfaces on top of our game program.</p>
                </div>

            </div>
        </main>
    );
}
