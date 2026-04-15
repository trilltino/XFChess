import { Calendar, User, ArrowRight } from 'lucide-react';
import { Link } from 'react-router-dom';

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
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><Calendar size={14} /> 1 Mar 2025</span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><User size={14} /> Tino</span>
                    </div>
                    <h3 style={{ fontSize: '1.8rem', marginBottom: '16px' }}>XFChess released!</h3>
                    <p style={{ color: 'var(--text-dim)', marginBottom: '24px', lineHeight: 1.6 }}>Experience the future of competitive chess on Solana with instant settlements and immutable game records.</p>
                    <Link to="/news/release" className="btn btn-secondary" style={{ width: 'auto', padding: '0 24px', display: 'inline-flex', alignItems: 'center', gap: '8px', textDecoration: 'none' }}>
                        Read more <ArrowRight size={16} />
                    </Link>
                </article>
            </div>
        </main>
    );
}
