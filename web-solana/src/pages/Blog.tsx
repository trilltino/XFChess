import { Calendar, User, ArrowRight } from 'lucide-react';

export function Blog() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <div className="section-label">DEVELOPMENT LOG</div>
            <h2 style={{ fontSize: '3rem' }}>XF Insights & News<span className="accent">.</span></h2>
            <p style={{ maxWidth: '700px', fontSize: '1.2rem', marginBottom: '48px' }}>
                Follow our journey in building the decentralized future of competitive chess.
            </p>

            <div className="blog-posts" style={{ display: 'grid', gap: '60px', marginTop: '48px' }}>
                <article style={{ borderBottom: '1px solid var(--border)', paddingBottom: '40px' }}>
                    <div style={{ display: 'flex', gap: '20px', alignItems: 'center', marginBottom: '16px', fontSize: '0.85rem', color: 'var(--text-dim)' }}>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><Calendar size={14} /> April 3, 2026</span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><User size={14} /> Core Dev Team</span>
                    </div>
                    <h3 style={{ fontSize: '1.8rem', marginBottom: '16px', cursor: 'pointer' }}>Mainnet Migration & Identity Vaulting Rollout</h3>
                    <p style={{ color: 'var(--text-dim)', marginBottom: '24px', lineHeight: 1.6 }}>We are officially migrating our CARF-compliant identity vault to the mainnet. This move enables full competitive wagered tournaments with secure, off-chain tax reporting capabilities.</p>
                    <button className="btn btn-secondary" style={{ width: 'auto', padding: '0 24px' }}>Read Post <ArrowRight size={16} /></button>
                </article>

                <article style={{ borderBottom: '1px solid var(--border)', paddingBottom: '40px' }}>
                    <div style={{ display: 'flex', gap: '20px', alignItems: 'center', marginBottom: '16px', fontSize: '0.85rem', color: 'var(--text-dim)' }}>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><Calendar size={14} /> March 28, 2026</span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><User size={14} /> Architecture Team</span>
                    </div>
                    <h3 style={{ fontSize: '1.8rem', marginBottom: '16px', cursor: 'pointer' }}>Glicko-2 Elo Rating On-Chain</h3>
                    <p style={{ color: 'var(--text-dim)', marginBottom: '24px', lineHeight: 1.6 }}>Our new ranking algorithm is now fully immutable on Solana. Your competitive rating is shared globally across any interface built on the XFChess program.</p>
                    <button className="btn btn-secondary" style={{ width: 'auto', padding: '0 24px' }}>Read Post <ArrowRight size={16} /></button>
                </article>
            </div>
        </main>
    );
}
