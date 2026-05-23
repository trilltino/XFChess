export function Learn() {
    const topics = [
        { title: 'Opening Principles', desc: 'Control the center, develop pieces, castle early.' },
        { title: 'Tactical Patterns', desc: 'Forks, pins, skewers, discovered attacks.' },
        { title: 'Endgame Basics', desc: 'King and pawn endings, rook endings.' },
        { title: 'XFChess on Solana', desc: 'How on-chain moves and session keys work.' },
    ];

    return (
        <div className="page-container" style={{ paddingTop: '120px', minHeight: '100vh', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            <div style={{ maxWidth: 720, width: '100%', padding: '0 24px' }}>
                <h1 style={{ fontSize: '2rem', fontWeight: 800, color: 'var(--text)', marginBottom: '12px' }}>
                    Learn
                </h1>
                <p style={{ color: 'var(--text-dim)', fontSize: '14px', marginBottom: '32px' }}>
                    Improve your game with guides, lessons, and XFChess tutorials.
                </p>
                <div style={{ display: 'grid', gap: '16px' }}>
                    {topics.map(t => (
                        <div key={t.title} style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '12px', padding: '24px' }}>
                            <h3 style={{ color: 'var(--text)', fontWeight: 700, marginBottom: '6px', fontSize: '15px' }}>{t.title}</h3>
                            <p style={{ color: 'var(--text-dim)', fontSize: '13px', margin: 0 }}>{t.desc}</p>
                        </div>
                    ))}
                </div>
                <p style={{ color: 'var(--text-dim)', fontSize: '12px', marginTop: '24px', textAlign: 'center' }}>
                    Full lesson content coming soon.
                </p>
            </div>
        </div>
    );
}
